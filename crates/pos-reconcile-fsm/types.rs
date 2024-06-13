use std::{collections::HashMap, num::NonZeroU128};

use num::{FixedU256, U256};

pub type Denom = String;
pub type FeeRecipient = String;
pub type StakeDenom = String;
pub type Validator = String;
pub type Account = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CurrentHeight(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Delegated(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct DelegateStartSlot(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeeBpsBlockIncrement(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeePaymentCooldownBlocks(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightDelegation(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightActualDelegation(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightDeposit(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightFeePayable(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightRewardsReceivable(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct InflightUnbond(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct LastReconcileHeight(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxFeeBps(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaxMsgCount(pub usize);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct MsgIssuedCount(pub usize);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct MsgSuccessCount(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Now(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct PendingDeposit(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct PendingUnbond(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct ReconcilerFee(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct RemoteBalance(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct RewardsReceivable(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbondedAmount(pub u128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbondingTimeSecs(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbondCompleteTimestamp(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct UndelegateStartSlot(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidatorSetSize(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct ValidatorSetSlot(pub usize);

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Delegation {
    pub validator: String,
    pub amount: u128,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct DelegationsIcqResult {
    pub last_submitted_height: u64,
    pub delegations: Vec<Delegation>,
}

/// A processed 'Delegations' ICQ
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
#[non_exhaustive]
pub struct DelegationsReport {
    pub height: u64,
    pub total_delegated: u128,
    // sorted to match validator set order
    pub delegated_amounts_per_slot: Vec<u128>,
}

impl DelegationsIcqResult {
    pub fn into_report(self, validators: &[Validator]) -> Option<DelegationsReport> {
        // map validator addresses to slot indexes
        let validator_to_slot: HashMap<_, _> = validators
            .iter()
            .enumerate()
            .map(|(idx, v)| (v.as_str(), idx))
            .collect();

        // filter out any delegations to validators not in the set (just in case of left over dust delegations)
        let mut filtered_delegations: Vec<_> = self
            .delegations
            .into_iter()
            .filter_map(|d| {
                validator_to_slot
                    .get(d.validator.as_str())
                    .map(|_| (d.validator, d.amount))
            })
            .collect();

        // the report is only valid if there is a delegation for every slot
        if filtered_delegations.len() != validators.len() {
            return None;
        }

        // sort the filtered delegations to be in the same order as the validator set
        filtered_delegations.sort_unstable_by_key(|(validator, _)| {
            validator_to_slot
                .get(validator.as_str())
                .expect("delegations only has expected validators")
        });

        // sum total delegated and construct delegated amount vector in one pass
        let (total_delegated, delegated_amounts_per_slot) = filtered_delegations.into_iter().fold(
            (0u128, Vec::with_capacity(validators.len())),
            |(total_delegated, mut delegation_amounts), (_, amount)| {
                delegation_amounts.push(amount);
                (total_delegated + amount, delegation_amounts)
            },
        );

        Some(DelegationsReport {
            height: self.last_submitted_height,
            total_delegated,
            delegated_amounts_per_slot,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeeBps(u64);

impl FeeBps {
    /// Applies the fee to the given `balance`, returning (`remaining_balance`, `fee_amount`).
    /// If the fee would consume the entire `balance`, it is ignored and the entire balance remains.
    pub fn apply_to(&self, balance: NonZeroU128) -> (NonZeroU128, Option<NonZeroU128>) {
        let Some(weight) = Weight::checked_from_bps(self.0) else {
            return (balance, None);
        };

        let fee_amount = weight.apply(balance.get());

        let Some(remaining_balance) = NonZeroU128::new(balance.get() - fee_amount) else {
            return (balance, None);
        };

        (remaining_balance, NonZeroU128::new(fee_amount))
    }
}

#[derive(Debug, Clone)]
pub struct FeeMetadata {
    pub fee_recipient: Option<FeeRecipient>,
    pub fee_payout_cooldown: FeePaymentCooldownBlocks,
    pub fee_bps_block_increment: FeeBpsBlockIncrement,
    pub max_fee_bps: MaxFeeBps,
}

impl FeeMetadata {
    pub fn fee_bps(
        &self,
        last_reconcile_height: LastReconcileHeight,
        current_height: CurrentHeight,
    ) -> Option<FeeBps> {
        if self.fee_recipient.is_none() || self.max_fee_bps.0 == 0 {
            return None;
        }

        let elapsed_height = last_reconcile_height.0.abs_diff(current_height.0);

        if self.fee_payout_cooldown.0 >= elapsed_height {
            return None;
        }

        let calculated_fee_bps =
            self.fee_bps_block_increment.0 * elapsed_height.abs_diff(self.fee_payout_cooldown.0);

        let actual_fee_bps = calculated_fee_bps.min(self.max_fee_bps.0);

        Some(FeeBps(actual_fee_bps))
    }
}

#[repr(u8)]
#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::IsVariant, derive_more::Display)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum Phase {
    #[default]
    #[display(fmt = "setup_rewards_address")]
    SetupRewardsAddress = 0,
    #[display(fmt = "setup_authz")]
    SetupAuthz = 1,
    #[display(fmt = "start_reconcile")]
    StartReconcile = 2,
    Redelegate = 3,
    Undelegate = 4,
    #[display(fmt = "transfer_undelegated")]
    TransferUndelegated = 5,
    #[display(fmt = "transfer_pending_deposits")]
    TransferPendingDeposits = 6,
    Delegate = 7,
}

impl Phase {
    // how many txs are issued by the phase in total
    pub fn tx_count(
        self,
        ValidatorSetSize(set_size): ValidatorSetSize,
        MaxMsgCount(max_msg_count): MaxMsgCount,
    ) -> usize {
        // in addition to one per slot
        let extra_msg_count = match self {
            // no messages other than `{Un, Re}delegate`
            Phase::Redelegate | Phase::Undelegate => 0,
            // extra messages required: one to send rewards + one to send fee
            Phase::Delegate => 2,
            // no need to continue in these cases:
            // no txs issued
            Phase::StartReconcile => return 0,
            // single tx issued
            Phase::TransferUndelegated
            | Phase::TransferPendingDeposits
            | Phase::SetupRewardsAddress
            | Phase::SetupAuthz => return 1,
        };

        let total_msg_count = set_size + extra_msg_count;

        let tx_count_floor = total_msg_count / max_msg_count;

        if total_msg_count % max_msg_count == 0 {
            return tx_count_floor;
        }

        tx_count_floor + 1
    }

    pub fn next(self) -> Option<Phase> {
        let next = match self {
            Phase::SetupRewardsAddress => Phase::SetupAuthz,
            Phase::SetupAuthz => Phase::StartReconcile,
            Phase::StartReconcile => Phase::Redelegate,
            Phase::Redelegate => Phase::Undelegate,
            Phase::Undelegate => Phase::TransferUndelegated,
            Phase::TransferUndelegated => Phase::TransferPendingDeposits,
            Phase::TransferPendingDeposits => Phase::Delegate,
            Phase::Delegate => return None,
        };

        Some(next)
    }

    pub fn sequence_tx_count(
        self,
        state: State,
        validator_set_size: ValidatorSetSize,
        max_msg_count: MaxMsgCount,
    ) -> usize {
        if state.is_pending() {
            return 0;
        }

        let mut count = self.tx_count(validator_set_size, max_msg_count);
        let mut phase = self;

        while let Some(next_phase) = phase.next() {
            phase = next_phase;
            count += phase.tx_count(validator_set_size, max_msg_count);
        }

        count
    }
}

impl From<Phase> for u8 {
    fn from(value: Phase) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for Phase {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::SetupRewardsAddress,
            1 => Self::SetupAuthz,
            2 => Self::StartReconcile,
            3 => Self::Redelegate,
            4 => Self::Undelegate,
            5 => Self::TransferUndelegated,
            6 => Self::TransferPendingDeposits,
            7 => Self::Delegate,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct RedelegationSlot(pub ValidatorSetSlot);

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(serde::Serialize))]
#[non_exhaustive]
pub struct RemoteBalanceReport {
    pub height: u64,
    pub amount: RemoteBalance,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::manual_non_exhaustive)]
pub struct UndelegatedBalanceReport {
    pub last_updated_timestamp: u64,
    pub remote_balance: RemoteBalanceReport,
}

pub struct BalancesIcqResult {
    pub last_submitted_height: u64,
    pub coins: Vec<(Denom, RemoteBalance)>,
}

impl BalancesIcqResult {
    pub fn into_remote_balance_report(self, stake_denom: &StakeDenom) -> RemoteBalanceReport {
        let amount = self
            .coins
            .iter()
            .find_map(|(denom, balance)| denom.eq(stake_denom.as_str()).then_some(*balance))
            .unwrap_or_default();

        RemoteBalanceReport {
            height: self.last_submitted_height,
            amount,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Default, Copy, Clone, PartialEq, derive_more::IsVariant, derive_more::Display)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum State {
    #[default]
    Idle = 0,
    Pending = 1,
    Failed = 2,
}

impl From<State> for u8 {
    fn from(value: State) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for State {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => State::Idle,
            1 => State::Pending,
            2 => State::Failed,
            _ => return Err(()),
        })
    }
}

/// A fixed point decimal always: 0 >= w <= 1.0
#[derive(Debug, Clone, Copy, PartialEq, derive_more::Display)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Weight(FixedU256);

impl Weight {
    pub const HUNDRED_PERCENT_BPS: u128 = 10_000; // 100 %

    pub const fn raw(raw: U256) -> Self {
        Self(FixedU256::raw(raw))
    }

    pub const fn into_raw(self) -> U256 {
        self.0.into_raw()
    }

    /// Returns `None` if `bps` > `Self::MAX_BPS`, otherwise `Some(Weight(bps / 10,000))`
    pub fn checked_from_bps(bps: impl Into<u128>) -> Option<Self> {
        let bps = bps.into();

        if bps > Self::HUNDRED_PERCENT_BPS {
            return None;
        }

        FixedU256::from_u128(bps)
            .checked_div(FixedU256::from_u128(Self::HUNDRED_PERCENT_BPS))
            .map(Self)
    }

    /// Returns `Some(weight)` if `numer <= denom` where the weight is `numer / denom`, otherwise `None`
    pub fn checked_from_fraction(numer: u128, denom: u128) -> Option<Self> {
        if numer > denom {
            return None;
        }

        if numer == 0 {
            return Some(Self(FixedU256::from_u128(0)));
        }

        FixedU256::from_u128(numer)
            .checked_div(FixedU256::from_u128(denom))
            .map(Self)
    }

    /// Apply the weight to `rhs` rounding towards zero
    pub fn apply(self, rhs: u128) -> u128 {
        if rhs == 0 {
            return 0;
        }

        self.0
            .checked_mul(FixedU256::from_u128(rhs))
            .expect("always: weight <= 1")
            .floor()
    }
}

/// The weights for all the validator slots, initially provided at instantiation
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Weights(Vec<Weight>);

pub trait SplitBalance {
    /// Splits the balance across a number of slots, returning `(total_allocated, Vec<allocations>)`
    /// `total_allocated` can be less than `balance` if there are rounding errors.
    /// `Vec<allocations>` will always be the same length as the number of slots, even if `balance == 0`.
    fn split_balance(&self, balance: u128) -> (u128, Vec<u128>);
}

impl Weights {
    pub fn new_unchecked(weights: Vec<Weight>) -> Self {
        Self(weights)
    }

    pub fn new(weights: &[Weight]) -> Option<Self> {
        // ensure at least one weight was provided
        if weights.is_empty() {
            return None;
        }

        let mut total = FixedU256::from_u128(0);

        for weight in weights {
            total = total.checked_add(weight.0)?;

            if total > FixedU256::from_u128(1) {
                return None;
            }
        }

        Some(Weights(weights.to_owned()))
    }

    pub fn as_slice(&self) -> &[Weight] {
        self.0.as_slice()
    }
}

fn split_balance_according_to_weight(weights: &[Weight], balance: u128) -> (u128, Vec<u128>) {
    if balance == 0 {
        return (0, vec![0; weights.len()]);
    }

    let mut allocations = Vec::with_capacity(weights.len());
    let mut total_allocated = 0u128;

    for weight in weights {
        let slot_balance = weight.apply(balance);

        allocations.push(slot_balance);

        total_allocated += slot_balance
    }

    assert!(
        total_allocated <= balance,
        "total allocated cannot be > total balance to split"
    );

    (total_allocated, allocations)
}

impl SplitBalance for Weights {
    fn split_balance(&self, balance: u128) -> (u128, Vec<u128>) {
        // no scaling is required, all weights are being used
        split_balance_according_to_weight(self.as_slice(), balance)
    }
}

impl<'a> SplitBalance for &'a Weights {
    fn split_balance(&self, balance: u128) -> (u128, Vec<u128>) {
        Weights::split_balance(self, balance)
    }
}

impl<'a> SplitBalance for &'a [Weight] {
    fn split_balance(&self, balance: u128) -> (u128, Vec<u128>) {
        // only a subset of weights might be being used, scaling is required so the sum of the weights is as close to 1.0 as possible
        let mut total_weight = FixedU256::zero();

        for weight in *self {
            total_weight = total_weight
                .checked_add(weight.0)
                .expect("always: weights total <= 1.0");
        }

        if total_weight.is_zero() {
            return (0, vec![0; self.len()]);
        }

        let mut scaled_weights = Vec::with_capacity(self.len());

        for weight in *self {
            let scaled_w = weight
                .0
                .checked_div(total_weight)
                .map(Weight)
                .expect("checked: total_weight > 0");

            scaled_weights.push(scaled_w);
        }

        split_balance_according_to_weight(&scaled_weights, balance)
    }
}
