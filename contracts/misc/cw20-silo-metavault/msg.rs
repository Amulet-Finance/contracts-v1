use cosmwasm_schema::{cw_serde, QueryResponses};

pub use amulet_cw::vault::{ExecuteMsg as VaultExecuteMsg, QueryMsg as VaultQueryMsg};
use cosmwasm_std::{Binary, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub cw20: String,
    pub hub: String,
    pub underlying_decimals: u32,
}

#[cw_serde]
pub struct MetadataResponse {
    pub owner: String,
    pub cw20: String,
    pub hub: String,
    pub deposits: Uint128,
}

#[cw_serde]
pub struct Cw20ReceiveMsg {
    pub sender: String,
    pub amount: Uint128,
    pub msg: Binary,
}

#[cw_serde]
pub enum Cw20Msg {
    Mint {},
}

#[cw_serde]
pub enum ProxyExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Redeem {},
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Vault(VaultExecuteMsg),
    Proxy(ProxyExecuteMsg),
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
    Vault(VaultQueryMsg),
    Strategy(StrategyQueryMsg),
}

impl From<ProxyExecuteMsg> for ExecuteMsg {
    fn from(v: ProxyExecuteMsg) -> Self {
        Self::Proxy(v)
    }
}

impl From<VaultExecuteMsg> for ExecuteMsg {
    fn from(v: VaultExecuteMsg) -> Self {
        Self::Vault(v)
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
