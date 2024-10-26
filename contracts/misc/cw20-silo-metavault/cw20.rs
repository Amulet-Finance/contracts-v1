use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, CosmosMsg, Deps, Env, StdError, Uint128, WasmMsg};

#[cw_serde]
pub(crate) enum QueryMsg {
    Balance { address: String },
    TokenInfo {},
}

#[cw_serde]
enum ExecuteMsg {
    Transfer { recipient: String, amount: Uint128 },
}

#[cw_serde]
pub(crate) struct BalanceResponse {
    pub balance: Uint128,
}

#[cw_serde]
pub(crate) struct TokenInfoResponse {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: Uint128,
}

pub fn balance(deps: Deps, env: &Env, cw20: &str) -> Result<Uint128, StdError> {
    let res: BalanceResponse = deps.querier.query_wasm_smart(
        cw20,
        &QueryMsg::Balance {
            address: env.contract.address.clone().into_string(),
        },
    )?;

    Ok(res.balance)
}

pub fn decimals(deps: Deps, cw20: &str) -> Result<u8, StdError> {
    let res: TokenInfoResponse = deps
        .querier
        .query_wasm_smart(cw20, &QueryMsg::TokenInfo {})?;

    Ok(res.decimals)
}

pub fn transfer<CustomMsg>(amount: u128, recipient: &str, cw20: &str) -> CosmosMsg<CustomMsg> {
    WasmMsg::Execute {
        contract_addr: cw20.to_owned(),
        msg: to_json_binary(&ExecuteMsg::Transfer {
            recipient: recipient.to_owned(),
            amount: amount.into(),
        })
        .expect("infallible serialization"),
        funds: vec![],
    }
    .into()
}
