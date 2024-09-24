// A 'blank' contract to replace a deprecated contract, ensuring that it can no longer be used.
// Sends all token balances to the address nominated in the `MigrateMsg`.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{entry_point, BankMsg, DepsMut, Env, MessageInfo, Response, StdError};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub struct MigrateMsg {
    token_receiver: String,
}

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, StdError> {
    deps.api.addr_validate(&msg.token_receiver)?;

    let balances = deps.querier.query_all_balances(env.contract.address)?;

    Ok(Response::default().add_message(BankMsg::Send {
        to_address: msg.token_receiver,
        amount: balances,
    }))
}
