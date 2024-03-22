use cosmwasm_schema::{cw_serde, QueryResponses};

pub use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    hub::{ExecuteMsg as HubExecuteMsg, QueryMsg as HubQueryMsg},
};

#[cw_serde]
pub struct InstantiateMsg {
    pub synthetic_mint: String,
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    Hub(HubExecuteMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Hub(HubQueryMsg),
}
