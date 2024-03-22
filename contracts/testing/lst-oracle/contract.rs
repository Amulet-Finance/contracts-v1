pub mod msg;

use anyhow::Error;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    Storage,
};

use amulet_cw::{strategy::generic_lst::RedemptionRateResponse, MigrateMsg, StorageExt as _};

use self::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

trait StorageExt: Storage {
    fn redemption_rate(&self) -> Decimal {
        self.u128_at("redemption_rate")
            .map(Decimal::raw)
            .unwrap_or(Decimal::one())
    }

    fn set_redemption_rate(&mut self, rate: Decimal) {
        self.set_u128("redemption_rate", rate.atomics().u128())
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, Error> {
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ExecuteMsg::SetRedemptionRate { rate } => deps.storage.set_redemption_rate(rate),
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
