use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    vault::{ExecuteMsg as VaultExecuteMsg, QueryMsg as VaultQueryMsg},
};

#[cw_serde]
pub struct InstantiateMsg {
    pub lst_redemption_rate_oracle: String,
    pub lst_denom: String,
    pub lst_decimals: u32,
    pub underlying_decimals: u32,
}

#[cw_serde]
pub struct MetadataResponse {
    pub lst_redemption_rate_oracle: String,
    pub lst_denom: String,
    pub lst_decimals: u32,
    pub underlying_decimals: u32,
    pub active_lst_balance: Uint128,
    pub claimable_lst_balance: Uint128,
}

#[cw_serde]
pub enum StrategyExecuteMsg {
    SetRedemptionRateOracle { oracle: String },
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    Vault(VaultExecuteMsg),
    Strategy(StrategyExecuteMsg),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum StrategyQueryMsg {
    #[returns(MetadataResponse)]
    Metadata {},
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Vault(VaultQueryMsg),
    Strategy(StrategyQueryMsg),
}
