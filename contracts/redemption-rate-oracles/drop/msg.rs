use cosmwasm_schema::cw_serde;

pub use amulet_cw::strategy::generic_lst::QueryMsg;
use cosmwasm_std::Decimal;

#[cw_serde]
pub struct InstantiateMsg {
    pub core_contract: String,
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum ExchangeRateQuery {
    #[returns(Decimal)]
    ExchangeRate {}
}