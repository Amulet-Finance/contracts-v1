pub mod msg;

use anyhow::{bail, Error};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Storage,
    Uint128, WasmMsg,
};

use amulet_cw::{hub::UserMsg as HubMsg, StorageExt as _};
use msg::{ConfigResponse, DepositAmountResponse, MetadataResponse};

use self::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

fn address_key(prefix: &str, address: &str) -> String {
    format!("{prefix}:{address}")
}

trait StorageExt: Storage {
    fn set_admin(&mut self, admin: &str) {
        self.set_string("admin", admin)
    }

    fn admin(&self) -> String {
        self.string_at("admin")
            .expect("always: set during initialisation")
    }

    fn set_hub(&mut self, address: &str) {
        self.set_string("hub", address)
    }

    fn hub(&self) -> String {
        self.string_at("hub")
            .expect("always: set during initialisation")
    }

    fn set_total_deposit_cap(&mut self, amount: Uint128) {
        self.set_u128("total_deposit_cap", amount.u128())
    }

    fn total_deposit_cap(&self) -> Uint128 {
        self.u128_at("total_deposit_cap")
            .map(Uint128::new)
            .expect("always: set during initialisation")
    }

    fn set_individual_deposit_cap(&mut self, amount: Uint128) {
        self.set_u128("individual_deposit_cap", amount.u128())
    }

    fn individual_deposit_cap(&self) -> Uint128 {
        self.u128_at("individual_deposit_cap")
            .map(Uint128::new)
            .expect("always: set during initialisation")
    }

    fn set_total_mint_cap(&mut self, amount: Uint128) {
        self.set_u128("total_mint_cap", amount.u128())
    }

    fn total_mint_cap(&self) -> Uint128 {
        self.u128_at("total_mint_cap")
            .map(Uint128::new)
            .expect("always: set during initialisation")
    }

    fn set_total_deposit_amount(&mut self, vault: &str, amount: Uint128) {
        self.set_u128(address_key("total_deposit_amount", vault), amount.u128())
    }

    fn total_deposit_amount(&self, vault: &str) -> Uint128 {
        self.u128_at(address_key("total_deposit_amount", vault))
            .map(Uint128::new)
            .unwrap_or_default()
    }

    fn set_individual_deposit_amount(&mut self, vault: &str, account: &str, amount: Uint128) {
        self.set_u128(
            address_key(&address_key("individual_deposit_amount", vault), account),
            amount.u128(),
        )
    }

    fn individual_deposit_amount(&self, vault: &str, account: &str) -> Uint128 {
        self.u128_at(address_key(
            &address_key("individual_deposit_amount", vault),
            account,
        ))
        .map(Uint128::new)
        .unwrap_or_default()
    }

    fn set_total_mint_amount(&mut self, vault: &str, amount: Uint128) {
        self.set_u128(address_key("total_mint_amount", vault), amount.u128())
    }

    fn total_mint_amount(&self, vault: &str) -> Uint128 {
        self.u128_at(address_key("total_mint_amount", vault))
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
    if let Some(admin) = msg.admin {
        deps.storage.set_admin(&admin);
    } else {
        deps.storage.set_admin(info.sender.as_str());
    }

    deps.storage.set_hub(&msg.hub_address);

    deps.storage.set_total_deposit_cap(msg.total_deposit_cap);

    deps.storage
        .set_individual_deposit_cap(msg.individual_deposit_cap);

    deps.storage.set_total_mint_cap(msg.total_mint_cap);

    Ok(Response::default())
}

pub fn handle_set_admin(
    deps: DepsMut,
    info: MessageInfo,
    address: &str,
) -> Result<Response, Error> {
    if info.sender != deps.storage.admin() {
        bail!("unauthorized")
    }

    deps.storage.set_admin(address);

    Ok(Response::default())
}

pub fn handle_set_config(
    deps: DepsMut,
    info: MessageInfo,
    individual_deposit_cap: Option<Uint128>,
    total_deposit_cap: Option<Uint128>,
    total_mint_cap: Option<Uint128>,
) -> Result<Response, Error> {
    if info.sender != deps.storage.admin() {
        bail!("unauthorized")
    }

    if let Some(amount) = individual_deposit_cap {
        deps.storage.set_individual_deposit_cap(amount)
    }

    if let Some(amount) = total_deposit_cap {
        deps.storage.set_total_deposit_cap(amount)
    }

    if let Some(amount) = total_mint_cap {
        deps.storage.set_total_mint_cap(amount)
    }

    Ok(Response::default())
}

pub fn handle_deposit(deps: DepsMut, info: MessageInfo, vault: String) -> Result<Response, Error> {
    let coin = cw_utils::one_coin(&info)?;

    let total_deposit_amount = deps
        .storage
        .total_deposit_amount(&vault)
        .strict_add(coin.amount);

    if total_deposit_amount > deps.storage.total_deposit_cap() {
        bail!("total deposit cap exceeded");
    }

    let individual_deposit_amount = deps
        .storage
        .individual_deposit_amount(&vault, info.sender.as_str())
        .strict_add(coin.amount);

    if individual_deposit_amount > deps.storage.individual_deposit_cap() {
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

    if total_mint_amount > deps.storage.total_mint_cap() {
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
        ExecuteMsg::SetAdmin { address } => handle_set_admin(deps, info, &address),

        ExecuteMsg::SetConfig {
            individual_deposit_cap,
            total_deposit_cap,
            total_mint_cap,
        } => handle_set_config(
            deps,
            info,
            individual_deposit_cap,
            total_deposit_cap,
            total_mint_cap,
        ),

        ExecuteMsg::Deposit { vault } => handle_deposit(deps, info, vault),

        ExecuteMsg::Mint { vault } => handle_mint(deps, info, vault),
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Config {} => to_json_binary(&ConfigResponse {
            admin: deps.storage.admin(),
            hub_address: deps.storage.hub(),
            individual_deposit_cap: deps.storage.individual_deposit_cap(),
            total_deposit_cap: deps.storage.total_deposit_cap(),
            total_mint_cap: deps.storage.total_mint_cap(),
        })?,

        QueryMsg::VaultMetadata { vault } => to_json_binary(&MetadataResponse {
            total_deposit: deps.storage.total_deposit_amount(&vault),
            total_mint: deps.storage.total_mint_amount(&vault),
        })?,

        QueryMsg::DepositAmount { vault, account } => to_json_binary(&DepositAmountResponse {
            amount: deps.storage.individual_deposit_amount(&vault, &account),
        })?,
    };

    Ok(binary)
}
