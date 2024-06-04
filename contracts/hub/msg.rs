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

impl From<HubExecuteMsg> for ExecuteMsg {
    fn from(v: HubExecuteMsg) -> Self {
        Self::Hub(v)
    }
}

impl From<AdminExecuteMsg> for ExecuteMsg {
    fn from(v: AdminExecuteMsg) -> Self {
        Self::Admin(v)
    }
}

impl From<HubQueryMsg> for QueryMsg {
    fn from(v: HubQueryMsg) -> Self {
        Self::Hub(v)
    }
}

impl From<AdminQueryMsg> for QueryMsg {
    fn from(v: AdminQueryMsg) -> Self {
        Self::Admin(v)
    }
}
