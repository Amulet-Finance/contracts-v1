use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

pub use amulet_cw::{
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

impl From<StrategyExecuteMsg> for ExecuteMsg {
    fn from(v: StrategyExecuteMsg) -> Self {
        Self::Strategy(v)
    }
}

impl From<VaultExecuteMsg> for ExecuteMsg {
    fn from(v: VaultExecuteMsg) -> Self {
        Self::Vault(v)
    }
}

impl From<AdminExecuteMsg> for ExecuteMsg {
    fn from(v: AdminExecuteMsg) -> Self {
        Self::Admin(v)
    }
}

impl From<StrategyQueryMsg> for QueryMsg {
    fn from(v: StrategyQueryMsg) -> Self {
        Self::Strategy(v)
    }
}

impl From<VaultQueryMsg> for QueryMsg {
    fn from(v: VaultQueryMsg) -> Self {
        Self::Vault(v)
    }
}

impl From<AdminQueryMsg> for QueryMsg {
    fn from(v: AdminQueryMsg) -> Self {
        Self::Admin(v)
    }
}
