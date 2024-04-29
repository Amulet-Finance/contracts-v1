use amulet_cw::StorageExt as _;
use cosmwasm_std::Storage;
use pos_reconcile_fsm::types::{
    Delegated, InflightDelegation, InflightDeposit, InflightFeePayable, InflightRewardsReceivable,
    InflightUnbond, LastReconcileHeight, MsgIssuedCount, MsgSuccessCount, PendingDeposit,
    PendingUnbond, Phase, State, Weight, Weights,
};

use crate::types::{AvailableToClaim, TotalActualUnbonded, TotalExpectedUnbonded};

#[rustfmt::skip]
mod key {
    use amulet_cw::MapKey;
    
    macro_rules! key {
        ($k:literal) => {
            concat!("remote_pos::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            amulet_cw::MapKey::new(key!($k))
        };
    }

    pub const AVAILABLE_TO_CLAIM: &str                    = key!("available_to_claim");
    pub const CONNECTION_ID: &str                         = key!("connection_id");
    pub const DELEGATED: &str                             = key!("delegated");
    pub const DELEGATIONS_ICQ: &str                       = key!("delegations_icq");
    pub const ESTIMATED_BLOCK_INTERVAL_SECONDS: &str      = key!("estimated_block_interval_seconds");
    pub const FEE_BPS_BLOCK_INCREMENT: &str               = key!("fee_bps_block_increment");
    pub const FEE_PAYMENT_COOLDOWN_BLOCKS: &str           = key!("fee_payment_cooldown_blocks");
    pub const FEE_RECIPIENT: &str                         = key!("fee_recipient");
    pub const IBC_DEPOSIT_ASSET: &str                     = key!("ibc_deposit_asset");
    pub const ICQ_UPDATE_INTERVAL: &str                   = key!("icq_update_interval");
    pub const INFLIGHT_DELEGATION: &str                   = key!("inflight_delegation");
    pub const INFLIGHT_DEPOSIT: &str                      = key!("inflight_deposit");
    pub const INFLIGHT_FEE_PAYABLE: &str                  = key!("inflight_fee_payable");
    pub const INFLIGHT_REWARDS_RECEIVABLE: &str           = key!("inflight_rewards_receivable");
    pub const INFLIGHT_UNBOND: &str                       = key!("inflight_unbond");
    pub const INTERCHAIN_TX_TIMEOUT_SECONDS: &str         = key!("interchain_tx_timeout_seconds");
    pub const LAST_RECONCILE_HEIGHT: &str                 = key!("last_reconcile_height");
    pub const LAST_UNBOND_TIMESTAMP: &str                 = key!("last_unbond_timestamp");
    pub const LAST_MAIN_ICA_BALANCE_ICQ_UPDATE: &str      = key!("last_main_ica_balance_icq_update");
    pub const LAST_USED_MAIN_ICA_BALANCE_ICQ_UPDATE: &str = key!("last_used_main_ica_balance_icq_update");
    pub const MAIN_ICA_ADDRESS: &str                      = key!("main_ica_address");
    pub const MAIN_ICA_BALANCE_ICQ: &str                  = key!("main_ica_balance_icq");
    pub const MAX_FEE_BPS: &str                           = key!("max_fee_bps");
    pub const MAX_IBC_MSG_COUNT: &str                     = key!("max_ibc_msg_count");
    pub const MAX_UNBONDING_ENTRIES: &str                 = key!("max_unbonding_entries");
    pub const MINIMUM_UNBOND_INTERVAL: &str               = key!("minimum_unbond_interval");
    pub const MSG_ISSUED_COUNT: &str                      = key!("msg_issued_count");
    pub const MSG_SUCCESS_COUNT: &str                     = key!("msg_success_count");
    pub const NEXT_DELEGATIONS_ICQ: &str                  = key!("next_delegations_icq");
    pub const PENDING_DEPOSIT: &str                       = key!("pending_deposit");
    pub const PENDING_UNBOND: &str                        = key!("pending_unbond");
    pub const RECONCILE_PHASE: &str                       = key!("reconcile_phase");
    pub const RECONCILE_STATE: &str                       = key!("reconcile_state");
    pub const RECONCILE_TRIGGER_ADDRESS: &str             = key!("reconcile_trigger_address");
    pub const REDELEGATE_SLOT: &str                       = key!("redelegate_slot");
    pub const REDELEGATE_TO: &str                         = key!("redelegate_to");
    pub const REMOTE_DENOM: &str                          = key!("remote_denom");
    pub const REMOTE_DENOM_DECIMALS: &str                 = key!("remote_denom_decimals");
    pub const REWARDS_ICA_ADDRESS: &str                   = key!("rewards_ica_address");
    pub const REWARDS_ICA_BALANCE_ICQ: &str               = key!("rewards_ica_balance_icq");
    pub const TOTAL_ACTUAL_UNBONDED: &str                 = key!("total_actual_unbonded");
    pub const TOTAL_EXPECTED_UNBONDED: &str               = key!("total_expected_unbonded");
    pub const TRANSFER_IN_CHANNEL: &str                   = key!("transfer_in_channel");
    pub const TRANSFER_IN_TIMEOUT_SECONDS: &str           = key!("transfer_in_timeout_seconds");
    pub const TRANSFER_OUT_CHANNEL: &str                  = key!("transfer_out_channel");
    pub const TRANSFER_OUT_TIMEOUT_SECONDS: &str          = key!("transfer_out_timeout_seconds");
    pub const UNBONDING_ACK_COUNT: &str                   = key!("unbonding_ack_count");
    pub const UNBONDING_EXPECTED_AMOUNT: MapKey           = map_key!("unbonding_expected_amount");
    pub const UNBONDING_ISSUED_COUNT: &str                = key!("unbonding_issued_count");
    pub const UNBONDING_LOCAL_EXPIRY: MapKey              = map_key!("unbonding_local_expiry");
    pub const UNBONDING_PERIOD: &str                      = key!("unbonding_period");
    pub const VALIDATOR: MapKey                           = map_key!("validator");
    pub const VALIDATOR_SET_SIZE: &str                    = key!("validator_set_size");
    pub const VALIDATOR_WEIGHT: MapKey                    = map_key!("validator_weight");
}

pub trait StorageExt: Storage {
    fn available_to_claim(&self) -> AvailableToClaim {
        self.u128_at(key::AVAILABLE_TO_CLAIM)
            .map(AvailableToClaim)
            .unwrap_or_default()
    }

    fn set_available_to_claim(&mut self, AvailableToClaim(amount): AvailableToClaim) {
        self.set_u128(key::AVAILABLE_TO_CLAIM, amount)
    }

    fn connection_id(&self) -> String {
        self.string_at(key::CONNECTION_ID)
            .expect("set during initialisation")
    }

    fn set_connection_id(&mut self, connection_id: &str) {
        self.set_string(key::CONNECTION_ID, connection_id);
    }

    fn delegated(&self) -> Delegated {
        self.u128_at(key::DELEGATED)
            .map(Delegated)
            .unwrap_or_default()
    }

    fn set_delegated(&mut self, Delegated(delegated): Delegated) {
        self.set_u128(key::DELEGATED, delegated)
    }

    fn delegations_icq(&self) -> Option<u64> {
        self.u64_at(key::DELEGATIONS_ICQ)
    }

    fn set_delegations_icq(&mut self, icq: u64) {
        self.set_u64(key::DELEGATIONS_ICQ, icq)
    }

    fn estimated_block_interval_seconds(&self) -> u64 {
        self.u64_at(key::ESTIMATED_BLOCK_INTERVAL_SECONDS)
            .expect("set during initialisation")
    }

    fn set_estimated_block_interval_seconds(&mut self, estimated_block_interval_seconds: u64) {
        self.set_u64(
            key::ESTIMATED_BLOCK_INTERVAL_SECONDS,
            estimated_block_interval_seconds,
        );
    }

    fn fee_bps_block_increment(&self) -> u64 {
        self.u64_at(key::FEE_BPS_BLOCK_INCREMENT)
            .expect("set during initialisation")
    }

    fn set_fee_bps_block_increment(&mut self, fee_bps_block_increment: u64) {
        self.set_u64(key::FEE_BPS_BLOCK_INCREMENT, fee_bps_block_increment);
    }

    fn fee_payment_cooldown_blocks(&self) -> u64 {
        self.u64_at(key::FEE_PAYMENT_COOLDOWN_BLOCKS)
            .expect("set during initialisation")
    }

    fn set_fee_payment_cooldown_blocks(&mut self, fee_payment_cooldown_blocks: u64) {
        self.set_u64(
            key::FEE_PAYMENT_COOLDOWN_BLOCKS,
            fee_payment_cooldown_blocks,
        );
    }

    fn fee_recipient(&self) -> Option<String> {
        let recipient = self.string_at(key::FEE_RECIPIENT)?;

        if recipient.is_empty() {
            return None;
        }

        Some(recipient)
    }

    fn set_fee_recipient(&mut self, fee_recipient: &str) {
        self.set_string(key::FEE_RECIPIENT, fee_recipient);
    }

    fn clear_fee_recipient(&mut self) {
        self.remove(key::FEE_RECIPIENT.as_bytes());
    }

    fn ibc_deposit_asset(&self) -> String {
        self.string_at(key::IBC_DEPOSIT_ASSET)
            .expect("set during initialisation")
    }

    fn set_ibc_deposit_asset(&mut self, denom: &str) {
        self.set_string(key::IBC_DEPOSIT_ASSET, denom);
    }

    fn icq_update_interval(&self) -> u64 {
        self.u64_at(key::ICQ_UPDATE_INTERVAL)
            .expect("set during initialisation")
    }

    fn set_icq_update_interval(&mut self, icq_update_interval: u64) {
        self.set_u64(key::ICQ_UPDATE_INTERVAL, icq_update_interval);
    }

    fn inflight_delegation(&self) -> InflightDelegation {
        self.u128_at(key::INFLIGHT_DELEGATION)
            .map(InflightDelegation)
            .unwrap_or_default()
    }

    fn set_inflight_delegation(
        &mut self,
        InflightDelegation(inflight_delegation): InflightDelegation,
    ) {
        self.set_u128(key::INFLIGHT_DELEGATION, inflight_delegation)
    }

    fn inflight_deposit(&self) -> InflightDeposit {
        self.u128_at(key::INFLIGHT_DEPOSIT)
            .map(InflightDeposit)
            .unwrap_or_default()
    }

    fn set_inflight_deposit(&mut self, InflightDeposit(inflight_deposit): InflightDeposit) {
        self.set_u128(key::INFLIGHT_DEPOSIT, inflight_deposit)
    }

    fn inflight_fee_payable(&self) -> InflightFeePayable {
        self.u128_at(key::INFLIGHT_FEE_PAYABLE)
            .map(InflightFeePayable)
            .unwrap_or_default()
    }

    fn set_inflight_fee_payable(
        &mut self,
        InflightFeePayable(inflight_fee_payable): InflightFeePayable,
    ) {
        self.set_u128(key::INFLIGHT_FEE_PAYABLE, inflight_fee_payable)
    }

    fn inflight_rewards_receivable(&self) -> InflightRewardsReceivable {
        self.u128_at(key::INFLIGHT_REWARDS_RECEIVABLE)
            .map(InflightRewardsReceivable)
            .unwrap_or_default()
    }

    fn set_inflight_rewards_receivable(
        &mut self,
        InflightRewardsReceivable(inflight_rewards_receivable): InflightRewardsReceivable,
    ) {
        self.set_u128(
            key::INFLIGHT_REWARDS_RECEIVABLE,
            inflight_rewards_receivable,
        )
    }

    fn inflight_unbond(&self) -> InflightUnbond {
        self.u128_at(key::INFLIGHT_UNBOND)
            .map(InflightUnbond)
            .unwrap_or_default()
    }

    fn set_inflight_unbond(&mut self, InflightUnbond(inflight_unbond): InflightUnbond) {
        self.set_u128(key::INFLIGHT_UNBOND, inflight_unbond)
    }

    fn interchain_tx_timeout_seconds(&self) -> u64 {
        self.u64_at(key::INTERCHAIN_TX_TIMEOUT_SECONDS)
            .expect("set during initialisation")
    }

    fn set_interchain_tx_timeout_seconds(&mut self, interchain_tx_timeout_seconds: u64) {
        self.set_u64(
            key::INTERCHAIN_TX_TIMEOUT_SECONDS,
            interchain_tx_timeout_seconds,
        );
    }

    fn last_unbond_timestamp(&self) -> Option<u64> {
        self.u64_at(key::LAST_UNBOND_TIMESTAMP)
    }

    fn set_last_unbond_timestamp(&mut self, last_unbond_timestamp: u64) {
        self.set_u64(key::LAST_UNBOND_TIMESTAMP, last_unbond_timestamp);
    }

    fn last_main_ica_balance_icq_update(&self) -> Option<u64> {
        self.u64_at(key::LAST_MAIN_ICA_BALANCE_ICQ_UPDATE)
    }

    fn set_last_main_ica_balance_icq_update(&mut self, timestamp: u64) {
        self.set_u64(key::LAST_MAIN_ICA_BALANCE_ICQ_UPDATE, timestamp);
    }

    fn last_used_main_ica_balance_icq_update(&self) -> Option<u64> {
        self.u64_at(key::LAST_USED_MAIN_ICA_BALANCE_ICQ_UPDATE)
    }

    fn set_last_used_main_ica_balance_icq_update(&mut self, timestamp: u64) {
        self.set_u64(key::LAST_USED_MAIN_ICA_BALANCE_ICQ_UPDATE, timestamp);
    }

    fn last_reconcile_height(&self) -> Option<LastReconcileHeight> {
        self.u64_at(key::LAST_RECONCILE_HEIGHT)
            .map(LastReconcileHeight)
    }

    fn set_last_reconcile_height(
        &mut self,
        LastReconcileHeight(last_reconcile_height): LastReconcileHeight,
    ) {
        self.set_u64(key::LAST_RECONCILE_HEIGHT, last_reconcile_height);
    }

    fn main_ica_address(&self) -> Option<String> {
        self.string_at(key::MAIN_ICA_ADDRESS)
    }

    fn set_main_ica_address(&mut self, address: &str) {
        self.set_string(key::MAIN_ICA_ADDRESS, address)
    }

    fn main_ica_balance_icq(&self) -> Option<u64> {
        self.u64_at(key::MAIN_ICA_BALANCE_ICQ)
    }

    fn set_main_ica_balance_icq(&mut self, icq: u64) {
        self.set_u64(key::MAIN_ICA_BALANCE_ICQ, icq)
    }

    fn max_fee_bps(&self) -> u32 {
        self.u32_at(key::MAX_FEE_BPS)
            .expect("set during initialisation")
    }

    fn set_max_fee_bps(&mut self, max_fee_bps: u32) {
        self.set_u32(key::MAX_FEE_BPS, max_fee_bps);
    }

    fn max_ibc_msg_count(&self) -> usize {
        self.usize_at(key::MAX_IBC_MSG_COUNT)
            .expect("set during initialisation")
    }

    fn set_max_ibc_msg_count(&mut self, max_ibc_msg_count: usize) {
        self.set_usize(key::MAX_IBC_MSG_COUNT, max_ibc_msg_count);
    }

    fn max_unbonding_entries(&self) -> u64 {
        self.u64_at(key::MAX_UNBONDING_ENTRIES)
            .expect("set during initialisation")
    }

    fn set_max_unbonding_entries(&mut self, max_unbonding_entries: u64) {
        self.set_u64(key::MAX_UNBONDING_ENTRIES, max_unbonding_entries);
    }

    fn minimum_unbond_interval(&self) -> u64 {
        self.u64_at(key::MINIMUM_UNBOND_INTERVAL)
            .expect("set during initialisation")
    }

    fn set_minimum_unbond_interval(&mut self, minimum_unbond_interval: u64) {
        self.set_u64(key::MINIMUM_UNBOND_INTERVAL, minimum_unbond_interval);
    }

    fn msg_issued_count(&self) -> MsgIssuedCount {
        self.usize_at(key::MSG_ISSUED_COUNT)
            .map(MsgIssuedCount)
            .unwrap_or_default()
    }

    fn set_msg_issued_count(&mut self, MsgIssuedCount(count): MsgIssuedCount) {
        self.set_usize(key::MSG_ISSUED_COUNT, count);
    }

    fn msg_success_count(&self) -> MsgSuccessCount {
        self.usize_at(key::MSG_SUCCESS_COUNT)
            .map(MsgSuccessCount)
            .unwrap_or_default()
    }

    fn set_msg_success_count(&mut self, MsgSuccessCount(count): MsgSuccessCount) {
        self.set_usize(key::MSG_SUCCESS_COUNT, count);
    }

    fn next_delegations_icq(&self) -> Option<u64> {
        self.u64_at(key::NEXT_DELEGATIONS_ICQ)
    }

    fn set_next_delegations_icq(&mut self, icq: u64) {
        self.set_u64(key::NEXT_DELEGATIONS_ICQ, icq)
    }

    fn pending_deposit(&self) -> PendingDeposit {
        self.u128_at(key::PENDING_DEPOSIT)
            .map(PendingDeposit)
            .unwrap_or_default()
    }

    fn set_pending_deposit(&mut self, PendingDeposit(pending_deposit): PendingDeposit) {
        self.set_u128(key::PENDING_DEPOSIT, pending_deposit)
    }

    fn pending_unbond(&self) -> PendingUnbond {
        self.u128_at(key::PENDING_UNBOND)
            .map(PendingUnbond)
            .unwrap_or_default()
    }

    fn set_pending_unbond(&mut self, PendingUnbond(pending_unbond): PendingUnbond) {
        self.set_u128(key::PENDING_UNBOND, pending_unbond)
    }

    fn reconcile_phase(&self) -> Phase {
        self.u8_at(key::RECONCILE_PHASE)
            .map(Phase::try_from)
            .transpose()
            .expect("always: valid phase stored")
            .unwrap_or_default()
    }

    fn set_reconcile_phase(&mut self, phase: Phase) {
        self.set_u8(key::RECONCILE_PHASE, phase as _);
    }

    fn reconcile_state(&self) -> State {
        self.u8_at(key::RECONCILE_STATE)
            .map(State::try_from)
            .transpose()
            .expect("always: valid state stored")
            .unwrap_or_default()
    }

    fn set_reconcile_state(&mut self, state: State) {
        self.set_u8(key::RECONCILE_STATE, state as _);
    }

    fn reconcile_trigger_address(&self) -> Option<String> {
        self.string_at(key::RECONCILE_TRIGGER_ADDRESS)
    }

    fn set_reconcile_trigger_address(&mut self, reconcile_trigger_address: &str) {
        self.set_string(key::RECONCILE_TRIGGER_ADDRESS, reconcile_trigger_address);
    }

    fn redelegate_slot(&self) -> Option<usize> {
        self.usize_at(key::REDELEGATE_SLOT)
    }

    fn set_redelegate_slot(&mut self, slot_idx: usize) {
        self.set_usize(key::REDELEGATE_SLOT, slot_idx)
    }

    fn clear_redelegate_slot(&mut self) {
        self.remove(key::REDELEGATE_SLOT.as_bytes())
    }

    fn redelegate_to(&self) -> Option<String> {
        self.string_at(key::REDELEGATE_TO)
    }

    fn set_redelegate_to(&mut self, to: &str) {
        self.set_string(key::REDELEGATE_TO, to)
    }

    fn clear_redelegate_to(&mut self) {
        self.remove(key::REDELEGATE_TO.as_bytes())
    }

    fn remote_denom(&self) -> String {
        self.string_at(key::REMOTE_DENOM)
            .expect("set during initialisation")
    }

    fn set_remote_denom(&mut self, remote_denom: &str) {
        self.set_string(key::REMOTE_DENOM, remote_denom);
    }

    fn remote_denom_decimals(&self) -> u32 {
        self.u32_at(key::REMOTE_DENOM_DECIMALS)
            .expect("set during initialisation")
    }

    fn set_remote_denom_decimals(&mut self, remote_denom_decimals: u32) {
        self.set_u32(key::REMOTE_DENOM_DECIMALS, remote_denom_decimals);
    }

    fn rewards_ica_address(&self) -> Option<String> {
        self.string_at(key::REWARDS_ICA_ADDRESS)
    }

    fn set_rewards_ica_address(&mut self, address: &str) {
        self.set_string(key::REWARDS_ICA_ADDRESS, address)
    }

    fn rewards_ica_balance_icq(&self) -> Option<u64> {
        self.u64_at(key::REWARDS_ICA_BALANCE_ICQ)
    }

    fn set_rewards_ica_balance_icq(&mut self, icq: u64) {
        self.set_u64(key::REWARDS_ICA_BALANCE_ICQ, icq)
    }

    fn total_actual_unbonded(&self) -> TotalActualUnbonded {
        self.u128_at(key::TOTAL_ACTUAL_UNBONDED)
            .map(TotalActualUnbonded)
            .unwrap_or_default()
    }

    fn set_total_actual_unbonded(
        &mut self,
        TotalActualUnbonded(total_actual_unbonded): TotalActualUnbonded,
    ) {
        self.set_u128(key::TOTAL_ACTUAL_UNBONDED, total_actual_unbonded)
    }

    fn total_expected_unbonded(&self) -> TotalExpectedUnbonded {
        self.u128_at(key::TOTAL_EXPECTED_UNBONDED)
            .map(TotalExpectedUnbonded)
            .unwrap_or_default()
    }

    fn set_total_expected_unbonded(
        &mut self,
        TotalExpectedUnbonded(total_expected_unbonded): TotalExpectedUnbonded,
    ) {
        self.set_u128(key::TOTAL_EXPECTED_UNBONDED, total_expected_unbonded)
    }

    fn transfer_in_channel(&self) -> String {
        self.string_at(key::TRANSFER_IN_CHANNEL)
            .expect("set during initialisation")
    }

    fn set_transfer_in_channel(&mut self, transfer_in_channel: &str) {
        self.set_string(key::TRANSFER_IN_CHANNEL, transfer_in_channel);
    }

    fn transfer_in_timeout_seconds(&self) -> u64 {
        self.u64_at(key::TRANSFER_IN_TIMEOUT_SECONDS)
            .expect("set during initialisation")
    }

    fn set_transfer_in_timeout_seconds(&mut self, transfer_in_timeout_seconds: u64) {
        self.set_u64(
            key::TRANSFER_IN_TIMEOUT_SECONDS,
            transfer_in_timeout_seconds,
        );
    }

    fn transfer_out_channel(&self) -> String {
        self.string_at(key::TRANSFER_OUT_CHANNEL)
            .expect("set during initialisation")
    }

    fn set_transfer_out_channel(&mut self, transfer_out_channel: &str) {
        self.set_string(key::TRANSFER_OUT_CHANNEL, transfer_out_channel);
    }

    fn transfer_out_timeout_seconds(&self) -> u64 {
        self.u64_at(key::TRANSFER_OUT_TIMEOUT_SECONDS)
            .expect("set during initialisation")
    }

    fn set_transfer_out_timeout_seconds(&mut self, transfer_out_timeout_seconds: u64) {
        self.set_u64(
            key::TRANSFER_OUT_TIMEOUT_SECONDS,
            transfer_out_timeout_seconds,
        );
    }

    fn unbonding_ack_count(&self) -> Option<u64> {
        self.u64_at(key::UNBONDING_ACK_COUNT)
    }

    fn set_unbonding_ack_count(&mut self, count: u64) {
        self.set_u64(key::UNBONDING_ACK_COUNT, count);
    }

    fn unbonding_expected_amount(&self, idx: u64) -> Option<u128> {
        self.u128_at(key::UNBONDING_EXPECTED_AMOUNT.with(idx))
    }

    fn set_unbonding_expected_amount(&mut self, idx: u64, amount: u128) {
        self.set_u128(key::UNBONDING_EXPECTED_AMOUNT.with(idx), amount);
    }

    fn unbonding_issued_count(&self) -> Option<u64> {
        self.u64_at(key::UNBONDING_ISSUED_COUNT)
    }

    fn set_unbonding_issued_count(&mut self, count: u64) {
        self.set_u64(key::UNBONDING_ISSUED_COUNT, count);
    }

    fn unbonding_local_expiry(&self, idx: u64) -> Option<u64> {
        self.u64_at(key::UNBONDING_LOCAL_EXPIRY.with(idx))
    }

    fn set_unbonding_local_expiry(&mut self, idx: u64, timestamp: u64) {
        self.set_u64(key::UNBONDING_LOCAL_EXPIRY.with(idx), timestamp);
    }

    fn unbonding_period(&self) -> u64 {
        self.u64_at(key::UNBONDING_PERIOD)
            .expect("set during initialisation")
    }

    fn set_unbonding_period(&mut self, unbonding_period: u64) {
        self.set_u64(key::UNBONDING_PERIOD, unbonding_period);
    }

    fn validator(&self, slot_idx: usize) -> String {
        self.string_at(key::VALIDATOR.with(slot_idx))
            .expect("set during initialisation")
    }

    fn validators(&self) -> Vec<String> {
        let set_size = self.validator_set_size();

        let mut validators = Vec::with_capacity(set_size);

        for slot_idx in 0..set_size {
            validators.push(self.validator(slot_idx));
        }

        validators
    }

    fn set_validator(&mut self, slot_idx: usize, validator: &str) {
        self.set_string(key::VALIDATOR.with(slot_idx), validator)
    }

    fn validator_set_size(&self) -> usize {
        self.usize_at(key::VALIDATOR_SET_SIZE)
            .expect("set during initialisation")
    }

    fn set_validator_set_size(&mut self, validator_set_size: usize) {
        self.set_usize(key::VALIDATOR_SET_SIZE, validator_set_size)
    }

    fn validator_weight(&self, slot_idx: usize) -> Weight {
        self.u256_at(key::VALIDATOR_WEIGHT.with(slot_idx))
            .map(Weight::raw)
            .expect("set during initialisation")
    }

    fn validator_weights(&self) -> Vec<Weight> {
        let set_size = self.validator_set_size();

        let mut weights = Vec::with_capacity(set_size);

        for slot_idx in 0..set_size {
            weights.push(self.validator_weight(slot_idx));
        }

        weights
    }

    fn set_validator_weight(&mut self, slot_idx: usize, validator_weight: Weight) {
        self.set_u256(
            key::VALIDATOR_WEIGHT.with(slot_idx),
            validator_weight.into_raw(),
        )
    }

    fn set_validator_weights(&mut self, weights: Weights) {
        for (idx, weight) in weights.iter().copied().enumerate() {
            self.set_validator_weight(idx, weight)
        }
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}
