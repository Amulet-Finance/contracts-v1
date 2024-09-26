pub mod msg;

use anyhow::{anyhow, bail, Error};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Storage,
    Uint128, WasmMsg,
};

use amulet_core::admin::AdminRole;
use amulet_cw::{
    admin::{self, Repository as AdminRepository},
    hub::UserMsg as HubMsg,
    StorageExt as _,
};

use self::msg::{
    ConfigResponse, DepositAmountResponse, ExecuteMsg, InstantiateMsg, MetadataResponse, ProxyMsg,
    ProxyQueryMsg, QueryMsg,
};

fn vault_not_found(vault: &str) -> Error {
    anyhow!("vault {vault} not found")
}

#[rustfmt::skip]
mod key {
    use amulet_cw::MapKey;

    macro_rules! key {
        ($k:literal) => {
            concat!("deposit_cap::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            MapKey::new(key!($k))
        };
    }

    pub const HUB                       : &str   = key!("hub");
    pub const TOTAL_DEPOSIT_CAP         : MapKey = map_key!("total_deposit_cap");
    pub const INDIVIDUAL_DEPOSIT_CAP    : MapKey = map_key!("individual_deposit_cap");
    pub const TOTAL_MINT_CAP            : MapKey = map_key!("total_mint_cap");
    pub const TOTAL_DEPOSIT_AMOUNT      : MapKey = map_key!("total_deposit_amount");
    pub const INDIVIDUAL_DEPOSIT_AMOUNT : MapKey = map_key!("individual_deposit_amount");
    pub const TOTAL_MINT_AMOUNT         : MapKey = map_key!("total_mint_amount");
}

trait StorageExt: Storage {
    fn set_hub(&mut self, address: &str) {
        self.set_string(key::HUB, address)
    }

    fn hub(&self) -> String {
        self.string_at(key::HUB)
            .expect("always: set during initialisation")
    }

    fn set_total_deposit_cap(&mut self, _: AdminRole, vault: &str, amount: Uint128) {
        self.set_u128(key::TOTAL_DEPOSIT_CAP.with(vault), amount.u128())
    }

    fn total_deposit_cap(&self, vault: &str) -> Option<Uint128> {
        self.u128_at(key::TOTAL_DEPOSIT_CAP.with(vault))
            .map(Uint128::new)
    }

    fn set_individual_deposit_cap(&mut self, _: AdminRole, vault: &str, amount: Uint128) {
        self.set_u128(key::INDIVIDUAL_DEPOSIT_CAP.with(vault), amount.u128())
    }

    fn individual_deposit_cap(&self, vault: &str) -> Option<Uint128> {
        self.u128_at(key::INDIVIDUAL_DEPOSIT_CAP.with(vault))
            .map(Uint128::new)
    }

    fn set_total_mint_cap(&mut self, _: AdminRole, vault: &str, amount: Uint128) {
        self.set_u128(key::TOTAL_MINT_CAP.with(vault), amount.u128())
    }

    fn total_mint_cap(&self, vault: &str) -> Option<Uint128> {
        self.u128_at(key::TOTAL_MINT_CAP.with(vault))
            .map(Uint128::new)
    }

    fn set_total_deposit_amount(&mut self, vault: &str, amount: Uint128) {
        self.set_u128(key::TOTAL_DEPOSIT_AMOUNT.with(vault), amount.u128())
    }

    fn total_deposit_amount(&self, vault: &str) -> Uint128 {
        self.u128_at(key::TOTAL_DEPOSIT_AMOUNT.with(vault))
            .map(Uint128::new)
            .unwrap_or_default()
    }

    fn set_individual_deposit_amount(&mut self, vault: &str, account: &str, amount: Uint128) {
        self.set_u128(
            key::INDIVIDUAL_DEPOSIT_AMOUNT.multi([&vault, &account]),
            amount.u128(),
        )
    }

    fn individual_deposit_amount(&self, vault: &str, account: &str) -> Uint128 {
        self.u128_at(key::INDIVIDUAL_DEPOSIT_AMOUNT.multi([&vault, &account]))
            .map(Uint128::new)
            .unwrap_or_default()
    }

    fn set_total_mint_amount(&mut self, vault: &str, amount: Uint128) {
        self.set_u128(key::TOTAL_MINT_AMOUNT.with(vault), amount.u128())
    }

    fn total_mint_amount(&self, vault: &str) -> Uint128 {
        self.u128_at(key::TOTAL_MINT_AMOUNT.with(vault))
            .map(Uint128::new)
            .unwrap_or_default()
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    deps.api.addr_validate(&msg.hub_address)?;

    deps.storage.set_hub(&msg.hub_address);

    Ok(Response::default())
}

pub fn handle_set_config(
    deps: DepsMut,
    info: MessageInfo,
    vault: &str,
    individual_deposit_cap: Option<Uint128>,
    total_deposit_cap: Option<Uint128>,
    total_mint_cap: Option<Uint128>,
) -> Result<Response, Error> {
    deps.api.addr_validate(vault)?;

    let admin_role = admin::get_admin_role(&AdminRepository::new(deps.storage), &info)?;

    if let Some(amount) = individual_deposit_cap {
        deps.storage
            .set_individual_deposit_cap(admin_role, vault, amount)
    }

    if let Some(amount) = total_deposit_cap {
        deps.storage
            .set_total_deposit_cap(admin_role, vault, amount)
    }

    if let Some(amount) = total_mint_cap {
        deps.storage.set_total_mint_cap(admin_role, vault, amount)
    }

    Ok(Response::default())
}

pub fn handle_deposit(deps: DepsMut, info: MessageInfo, vault: String) -> Result<Response, Error> {
    deps.api.addr_validate(&vault)?;

    let coin = cw_utils::one_coin(&info)?;

    let total_deposit_amount = deps
        .storage
        .total_deposit_amount(&vault)
        .strict_add(coin.amount);

    let (total_deposit_cap, individual_deposit_cap) = deps
        .storage
        .total_deposit_cap(&vault)
        .zip(deps.storage.individual_deposit_cap(&vault))
        .ok_or_else(|| vault_not_found(&vault))?;

    if total_deposit_amount > total_deposit_cap {
        bail!("total deposit cap exceeded");
    }

    let individual_deposit_amount = deps
        .storage
        .individual_deposit_amount(&vault, info.sender.as_str())
        .strict_add(coin.amount);

    if individual_deposit_amount > individual_deposit_cap {
        bail!("individual deposit cap exceeded");
    }

    deps.storage
        .set_total_deposit_amount(&vault, total_deposit_amount);

    deps.storage.set_individual_deposit_amount(
        &vault,
        info.sender.as_str(),
        individual_deposit_amount,
    );

    Ok(Response::default().add_message(WasmMsg::Execute {
        contract_addr: deps.storage.hub(),
        msg: to_json_binary(&HubMsg::DepositOnBehalf {
            vault,
            behalf_of: info.sender.into_string(),
        })?,
        funds: info.funds,
    }))
}

pub fn handle_mint(deps: DepsMut, info: MessageInfo, vault: String) -> Result<Response, Error> {
    let coin = cw_utils::one_coin(&info)?;

    let total_mint_amount = deps
        .storage
        .total_mint_amount(&vault)
        .strict_add(coin.amount);

    let total_mint_cap = deps
        .storage
        .total_mint_cap(&vault)
        .ok_or_else(|| vault_not_found(&vault))?;

    if total_mint_amount > total_mint_cap {
        bail!("total mint cap exceeded");
    }

    deps.storage
        .set_total_mint_amount(&vault, total_mint_amount);

    Ok(Response::default().add_message(WasmMsg::Execute {
        contract_addr: deps.storage.hub(),
        msg: to_json_binary(&HubMsg::MintOnBehalf {
            vault,
            behalf_of: info.sender.into_string(),
        })?,
        funds: info.funds,
    }))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ExecuteMsg::Admin(msg) => {
            let cmd = admin::handle_execute_msg(
                deps.api,
                &AdminRepository::new(deps.storage),
                info,
                msg,
            )?;

            admin::handle_cmd(deps.storage, cmd);

            Ok(Response::default())
        }

        ExecuteMsg::Proxy(ProxyMsg::SetConfig {
            vault,
            individual_deposit_cap,
            total_deposit_cap,
            total_mint_cap,
        }) => handle_set_config(
            deps,
            info,
            &vault,
            individual_deposit_cap,
            total_deposit_cap,
            total_mint_cap,
        ),

        ExecuteMsg::Proxy(ProxyMsg::Deposit { vault }) => handle_deposit(deps, info, vault),

        ExecuteMsg::Proxy(ProxyMsg::Mint { vault }) => handle_mint(deps, info, vault),
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(query) => {
            admin::handle_query_msg(&AdminRepository::new(deps.storage), query)?
        }

        QueryMsg::Proxy(ProxyQueryMsg::Config { vault }) => {
            let ((individual_deposit_cap, total_deposit_cap), total_mint_cap) = deps
                .storage
                .individual_deposit_cap(&vault)
                .zip(deps.storage.total_deposit_cap(&vault))
                .zip(deps.storage.total_mint_cap(&vault))
                .ok_or_else(|| vault_not_found(&vault))?;

            to_json_binary(&ConfigResponse {
                hub_address: deps.storage.hub(),
                individual_deposit_cap,
                total_deposit_cap,
                total_mint_cap,
            })?
        }

        QueryMsg::Proxy(ProxyQueryMsg::VaultMetadata { vault }) => {
            to_json_binary(&MetadataResponse {
                total_deposit: deps.storage.total_deposit_amount(&vault),
                total_mint: deps.storage.total_mint_amount(&vault),
            })?
        }

        QueryMsg::Proxy(ProxyQueryMsg::DepositAmount { vault, account }) => {
            to_json_binary(&DepositAmountResponse {
                amount: deps.storage.individual_deposit_amount(&vault, &account),
            })?
        }
    };

    Ok(binary)
}
