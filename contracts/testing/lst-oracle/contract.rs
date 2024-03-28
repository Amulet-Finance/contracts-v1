pub mod msg;

use anyhow::{bail, Error};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    Storage,
};

use amulet_cw::{strategy::generic_lst::RedemptionRateResponse, MigrateMsg, StorageExt as _};

use self::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

fn address_key(prefix: &str, address: &str) -> String {
    format!("{prefix}:{address}")
}

trait StorageExt: Storage {
    fn creator(&self) -> String {
        self.string_at("creator")
            .expect("always: set during initialisation")
    }

    fn redemption_rate(&self) -> Decimal {
        self.u128_at("redemption_rate")
            .map(Decimal::raw)
            .unwrap_or(Decimal::one())
    }

    fn is_whitelisted(&self, address: &str) -> bool {
        self.bool_at(address_key("whitelist", address))
            .unwrap_or_default()
    }

    fn set_creator(&mut self, creator: &str) {
        self.set_string("creator", creator)
    }

    fn set_redemption_rate(&mut self, rate: Decimal) {
        self.set_u128("redemption_rate", rate.atomics().u128())
    }

    fn set_whitelisted(&mut self, address: &str, whitelisted: bool) {
        self.set_bool(address_key("whitelist", address), whitelisted);
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, Error> {
    deps.storage.set_creator(info.sender.as_str());

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ExecuteMsg::SetWhitelisted {
            address,
            whitelisted,
        } => {
            if info.sender != deps.storage.creator() {
                bail!("unauthorized")
            }

            deps.api.addr_validate(&address)?;

            deps.storage.set_whitelisted(&address, whitelisted);
        }

        ExecuteMsg::SetRedemptionRate { rate } => {
            if info.sender != deps.storage.creator()
                && !deps.storage.is_whitelisted(info.sender.as_str())
            {
                bail!("unauthorized")
            }

            deps.storage.set_redemption_rate(rate)
        }
    }

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::RedemptionRate {} => to_json_binary(&RedemptionRateResponse {
            rate: deps.storage.redemption_rate(),
        })?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}
