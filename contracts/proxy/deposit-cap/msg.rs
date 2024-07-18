use cosmwasm_schema::{cw_serde, QueryResponses};

use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub hub_address: String,
    pub individual_deposit_cap: Uint128,
    pub total_deposit_cap: Uint128,
    pub total_mint_cap: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    SetAdmin {
        address: String,
    },
    SetConfig {
        individual_deposit_cap: Option<Uint128>,
        total_deposit_cap: Option<Uint128>,
        total_mint_cap: Option<Uint128>,
    },
    Deposit {
        vault: String,
    },
    Mint {
        vault: String,
    },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: String,
    pub hub_address: String,
    pub individual_deposit_cap: Uint128,
    pub total_deposit_cap: Uint128,
    pub total_mint_cap: Uint128,
}

#[cw_serde]
pub struct MetadataResponse {
    pub total_deposit: Uint128,
    pub total_mint: Uint128,
}

#[cw_serde]
pub struct DepositAmountResponse {
    pub amount: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(MetadataResponse)]
    VaultMetadata { vault: String },
    #[returns(DepositAmountResponse)]
    DepositAmount { vault: String, account: String },
}
