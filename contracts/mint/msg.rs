use cosmwasm_schema::{cw_serde, QueryResponses};

pub use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    mint::{ExecuteMsg as MintExecuteMsg, QueryMsg as MintQueryMsg},
};
use amulet_token_factory::Flavour as TokenFactoryFlavour;

#[cw_serde]
pub struct InstantiateMsg {
    pub token_factory_flavour: TokenFactoryFlavour,
}

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

impl From<MintExecuteMsg> for ExecuteMsg {
    fn from(v: MintExecuteMsg) -> Self {
        Self::Mint(v)
    }
}

impl From<AdminExecuteMsg> for ExecuteMsg {
    fn from(v: AdminExecuteMsg) -> Self {
        Self::Admin(v)
    }
}

impl From<MintQueryMsg> for QueryMsg {
    fn from(v: MintQueryMsg) -> Self {
        Self::Mint(v)
    }
}

impl From<AdminQueryMsg> for QueryMsg {
    fn from(v: AdminQueryMsg) -> Self {
        Self::Admin(v)
    }
}
