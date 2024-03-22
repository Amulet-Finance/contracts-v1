use cosmwasm_schema::cw_serde;

pub use amulet_cw::strategy::generic_lst::QueryMsg;
use cosmwasm_std::Decimal;

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    SetRedemptionRate { rate: Decimal },
}
