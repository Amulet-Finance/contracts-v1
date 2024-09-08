use cosmwasm_schema::{cw_serde, QueryResponses};

use cosmwasm_std::Uint128;

pub use amulet_cw::admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg};

#[cw_serde]
pub struct InstantiateMsg {
    pub hub_address: String,
}

#[cw_serde]
pub enum ProxyExecuteMsg {
    SetWhitelisted { address: String, whitelisted: bool },
    Deposit { vault: String },
    Mint { vault: String },
    Advance { vault: String, amount: Uint128 },
    Redeem { vault: String },
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    Proxy(ProxyExecuteMsg),
}

#[cw_serde]
pub struct ConfigResponse {
    pub hub_address: String,
}

#[cw_serde]
pub struct WhitelistedResponse {
    pub whitelisted: bool,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ProxyQueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(WhitelistedResponse)]
    Whitelisted { address: String },
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Proxy(ProxyQueryMsg),
}
