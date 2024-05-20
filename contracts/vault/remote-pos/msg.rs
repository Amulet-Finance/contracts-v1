use cosmwasm_schema::{cw_serde, QueryResponses};

use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    vault::{ExecuteMsg as VaultExecuteMsg, QueryMsg as VaultQueryMsg},
};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct Config {
    pub connection_id: String,
    pub estimated_block_interval_seconds: u64,
    pub fee_bps_block_increment: u64,
    pub fee_payment_cooldown_blocks: u64,
    pub icq_update_interval: u64,
    pub interchain_tx_timeout_seconds: u64,
    pub max_fee_bps: u32,
    pub max_unbonding_entries: u64,
    pub remote_denom: String,
    pub remote_denom_decimals: u32,
    pub transfer_in_channel: String,
    pub transfer_in_timeout_seconds: u64,
    pub transfer_out_channel: String,
    pub transfer_out_timeout_seconds: u64,
    pub unbonding_period: u64,
}

#[cw_serde]
pub struct InstantiateMsg {
    #[serde(flatten)]
    pub config: Config,
    pub initial_validator_set: Vec<String>,
    pub initial_validator_weights: Vec<u32>,
}

#[cw_serde]
pub struct Metadata {
    pub available_to_claim: Uint128,
    pub delegated: Uint128,
    pub delegations_icq: Option<u64>,
    pub ibc_deposit_asset: String,
    pub inflight_delegation: Uint128,
    pub inflight_deposit: Uint128,
    pub inflight_fee_payable: Uint128,
    pub inflight_rewards_receivable: Uint128,
    pub inflight_unbond: Uint128,
    pub last_reconcile_height: Option<u64>,
    pub last_unbond_timestamp: Option<u64>,
    pub last_main_ica_balance_icq_update: Option<u64>,
    pub last_used_main_ica_balance_icq_update: Option<u64>,
    pub main_ica_address: Option<String>,
    pub main_ica_balance_icq: Option<u64>,
    pub max_ibc_msg_count: usize,
    pub minimum_unbond_interval: u64,
    pub msg_issued_count: usize,
    pub msg_success_count: usize,
    pub pending_deposit: Uint128,
    pub pending_unbond: Uint128,
    pub rewards_ica_address: Option<String>,
    pub rewards_ica_balance_icq: Option<u64>,
    pub total_actual_unbonded: Uint128,
    pub total_expected_unbonded: Uint128,
    pub unbonding_ack_count: Option<u64>,
    pub unbonding_issued_count: Option<u64>,
}

#[cw_serde]
pub struct ReconcileState {
    pub fee_recipient: Option<String>,
    pub phase: String,
    pub state: String,
    pub trigger_address: Option<String>,
    pub cost: Uint128,
}

#[cw_serde]
pub struct ValidatorSet {
    pub size: usize,
    pub validators: Vec<String>,
    pub weights: Vec<String>,
}

#[cw_serde]
pub enum StrategyExecuteMsg {
    /// Force a failed phase to continue to the next phase, if elligible
    ForceNext {},
    Reconcile {
        fee_recipient: Option<String>,
    },
    ReceiveUndelegated {},
    /// Admin role required
    RedelegateSlot {
        slot: usize,
        validator: String,
    },
    /// Restore an ICA - requires submitting the registration fee
    /// Note that this will fail if the channel is open.
    RestoreIca {
        id: String,
    },
    /// Restore an ICQ channel - requires submitting the deposit
    /// Note that this will fail if the channel is open.
    RestoreIcq {
        id: String,
    },
    /// Query the x/interchain-txs params for the max msg count and reset cached value
    ResetMaxMsgCount {},
    /// Admin role required
    UpdateConfig {
        estimated_block_interval_seconds: Option<u64>,
        fee_bps_block_increment: Option<u64>,
        fee_payment_cooldown_blocks: Option<u64>,
        icq_update_interval: Option<u64>,
        interchain_tx_timeout_seconds: Option<u64>,
        max_fee_bps: Option<u32>,
        transfer_in_timeout_seconds: Option<u64>,
        transfer_out_timeout_seconds: Option<u64>,
    },
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
    #[returns(Config)]
    Config {},
    #[returns(Metadata)]
    Metadata {},
    #[returns(ReconcileState)]
    ReconcileState {},
    #[returns(ValidatorSet)]
    ValidatorSet {},
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
