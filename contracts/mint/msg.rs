use cosmwasm_schema::{cw_serde, QueryResponses};

pub use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    mint::{ExecuteMsg as MintExecuteMsg, QueryMsg as MintQueryMsg},
};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    Mint(MintExecuteMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Mint(MintQueryMsg),
}
