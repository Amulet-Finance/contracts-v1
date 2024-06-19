pub mod types;

use std::{collections::BTreeMap, num::NonZeroU128};

use num::FixedU256;
use types::{
    Account, CurrentHeight, DelegateStartSlot, Delegated, DelegationsReport, FeeBpsBlockIncrement,
    FeeMetadata, FeePaymentCooldownBlocks, FeeRecipient, InflightDelegation, InflightDeposit,
    InflightFeePayable, InflightRewardsReceivable, InflightUnbond, LastReconcileHeight, MaxFeeBps,
    MaxMsgCount, MsgIssuedCount, MsgSuccessCount, Now, PendingDeposit, PendingUnbond, Phase,
    ReconcilerFee, RedelegationSlot, RemoteBalance, RemoteBalanceReport, RewardsReceivable, State,
    UnbondingTimeSecs, UndelegateStartSlot, UndelegatedBalanceReport, ValidatorSetSize,
    ValidatorSetSlot, Weight, Weights,
};

/// Access fixed config
pub trait Config {
    fn unbonding_time(&self) -> UnbondingTimeSecs;

    fn max_msg_count(&self) -> MaxMsgCount;

    fn fee_payout_cooldown(&self) -> FeePaymentCooldownBlocks;

    fn fee_bps_block_increment(&self) -> FeeBpsBlockIncrement;

    fn max_fee_bps(&self) -> MaxFeeBps;

    fn starting_weights(&self) -> Weights;

    fn validator_set_size(&self) -> ValidatorSetSize;
}

/// Access mutable storage
pub trait Repository {
    fn delegated(&self) -> Delegated;

    fn delegate_start_slot(&self) -> DelegateStartSlot;

    fn inflight_delegation(&self) -> InflightDelegation;

    fn inflight_deposit(&self) -> InflightDeposit;

    fn inflight_fee_payable(&self) -> InflightFeePayable;

    fn inflight_rewards_receivable(&self) -> InflightRewardsReceivable;

    fn inflight_unbond(&self) -> InflightUnbond;

    fn last_reconcile_height(&self) -> Option<LastReconcileHeight>;

    fn msg_issued_count(&self) -> MsgIssuedCount;

    fn msg_success_count(&self) -> MsgSuccessCount;

    fn pending_deposit(&self) -> PendingDeposit;

    fn pending_unbond(&self) -> PendingUnbond;

    fn phase(&self) -> Phase;

    fn state(&self) -> State;

    fn redelegation_slot(&self) -> Option<RedelegationSlot>;

    fn undelegate_start_slot(&self) -> UndelegateStartSlot;

    fn weights(&self) -> Weights;
}

/// Access current environment
pub trait Env {
    fn current_height(&self) -> CurrentHeight;

    fn now(&self) -> Now;

    fn delegation_account_address(&self) -> Option<Account>;

    fn rewards_account_address(&self) -> Option<Account>;

    fn fee_recipient(&self) -> Option<FeeRecipient>;

    fn delegations_report(&self) -> Option<DelegationsReport>;

    fn rewards_balance_report(&self) -> Option<RemoteBalanceReport>;

    fn undelegated_balance_report(&self) -> Option<UndelegatedBalanceReport>;
}

fn fee_metadata(config: &dyn Config, env: &dyn Env) -> FeeMetadata {
    FeeMetadata {
        fee_recipient: env.fee_recipient(),
        fee_payout_cooldown: config.fee_payout_cooldown(),
        fee_bps_block_increment: config.fee_bps_block_increment(),
        max_fee_bps: config.max_fee_bps(),
    }
}

/// The types of message that can be issued in a single Authz exec message
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum AuthzMsg {
    SendRewardsReceivable(RewardsReceivable),
    SendFee(FeeRecipient, ReconcilerFee),
}

/// The types of message that are issued in interchain txs, as an IBC transfer or locally on the host chain
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum TxMsg {
    SetRewardsWithdrawalAddress(Account, Account),
    GrantAuthzSend(Account, Account),
    TransferInUndelegated(u128),
    TransferOutPendingDeposit(u128),
    WithdrawRewards(ValidatorSetSlot),
    Redelegate(ValidatorSetSlot, u128),
    Undelegate(ValidatorSetSlot, u128),
    Delegate(ValidatorSetSlot, u128),
    Authz(Vec<AuthzMsg>),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
/// Guaranteed to hold at least one tx message
#[non_exhaustive]
pub struct TxMsgs {
    pub msgs: Vec<TxMsg>,
}

impl TxMsgs {
    pub fn new(msgs: Vec<TxMsg>) -> Option<Self> {
        if msgs.is_empty() {
            return None;
        }

        Some(Self { msgs })
    }

    pub fn single(msg: TxMsg) -> Self {
        Self { msgs: vec![msg] }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
/// Commands used to update the mutable state
pub enum Cmd {
    ClearRedelegationRequest,
    Delegated(Delegated),
    DelegateStartSlot(DelegateStartSlot),
    InflightDelegation(InflightDelegation),
    InflightDeposit(InflightDeposit),
    InflightFeePayable(InflightFeePayable),
    InflightRewardsReceivable(InflightRewardsReceivable),
    InflightUnbond(InflightUnbond),
    LastReconcileHeight(LastReconcileHeight),
    MsgIssuedCount(MsgIssuedCount),
    MsgSuccessCount(MsgSuccessCount),
    PendingDeposit(PendingDeposit),
    PendingUnbond(PendingUnbond),
    Phase(Phase),
    State(State),
    UndelegateStartSlot(UndelegateStartSlot),
    Weights(Weights),
}

macro_rules! set {
    ($($item:expr),+) => {
        vec![
            $($item.into()),+
        ]
    };
}

macro_rules! impl_cmd_from {
    ($($t:ident),+) => {
        $(
            impl From<$t> for Cmd {
                fn from(v: $t) -> Self {
                    Self::$t(v)
                }
            }
        )+
    };
}

impl_cmd_from![
    Delegated,
    DelegateStartSlot,
    InflightDeposit,
    InflightDelegation,
    InflightFeePayable,
    InflightRewardsReceivable,
    InflightUnbond,
    LastReconcileHeight,
    MsgIssuedCount,
    MsgSuccessCount,
    PendingDeposit,
    PendingUnbond,
    Phase,
    State,
    UndelegateStartSlot,
    Weights
];

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(serde::Serialize))]
/// Events that occur during the state machine execution
pub enum Event {
    SlashDetected(FixedU256),
    UndelegatedAssetsTransferred,
    DepositsTransferred(u128),
    UnbondStarted(u128),
    DelegationsIncreased(u128),
    RedelegationSuccessful,
}

#[cfg_attr(test, derive(serde::Serialize))]
pub struct Response {
    pub cmds: Vec<Cmd>,
    pub events: Vec<Event>,
    pub tx_msgs: Option<TxMsgs>,
    pub tx_skip_count: usize,
}

struct TxMsgBatcher {
    msgs_success: MsgSuccessCount,
    max_msg_count: MaxMsgCount,
}

impl TxMsgBatcher {
    fn new(config: &dyn Config, repo: &dyn Repository) -> Self {
        Self {
            msgs_success: repo.msg_success_count(),
            max_msg_count: config.max_msg_count(),
        }
    }

    fn sent_msg_count(&self) -> usize {
        self.msgs_success.0
    }

    fn max_msg_count(&self) -> usize {
        self.max_msg_count.0
    }

    fn batch_msgs(&self, msgs: impl IntoIterator<Item = TxMsg>) -> Option<TxMsgs> {
        let msg_batch = msgs
            .into_iter()
            .skip(self.sent_msg_count())
            .take(self.max_msg_count())
            .collect();

        TxMsgs::new(msg_batch)
    }
}

trait EnvExt: Env {
    fn rewards_balance(
        &self,
        LastReconcileHeight(last_reconcile_height): LastReconcileHeight,
    ) -> Option<NonZeroU128> {
        let rewards_balance_report = self.rewards_balance_report()?;

        if rewards_balance_report.height <= last_reconcile_height {
            return None;
        }

        let RemoteBalance(amount) = rewards_balance_report.amount;

        NonZeroU128::new(amount)
    }
}

impl<T> EnvExt for T where T: Env + ?Sized {}

pub trait Fsm {
    fn reconcile(&self) -> Response;

    fn failed(&self) -> Response;

    fn force_next(&self) -> Option<Response>;
}

#[derive(Clone, Copy)]
struct Context<'a> {
    config: &'a dyn Config,
    repo: &'a dyn Repository,
    env: &'a dyn Env,
}

#[derive(Clone, Copy)]
pub struct FsmImpl<'a> {
    ctx: Context<'a>,
}

pub fn fsm<'a>(config: &'a dyn Config, repo: &'a dyn Repository, env: &'a dyn Env) -> FsmImpl<'a> {
    FsmImpl {
        ctx: Context { config, repo, env },
    }
}

enum TransitionKind {
    Abort,
    Next,
    Tx(TxMsgs),
}

struct Transition {
    kind: TransitionKind,
    cmds: Vec<Cmd>,
    events: Vec<Event>,
}

impl Transition {
    const fn next(cmds: Vec<Cmd>) -> Self {
        Self {
            kind: TransitionKind::Next,
            cmds,
            events: vec![],
        }
    }

    const fn tx(tx_msgs: TxMsgs, cmds: Vec<Cmd>) -> Transition {
        Self {
            kind: TransitionKind::Tx(tx_msgs),
            cmds,
            events: vec![],
        }
    }

    const fn abort() -> Transition {
        Self {
            kind: TransitionKind::Abort,
            cmds: vec![],
            events: vec![],
        }
    }

    fn event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }
}

#[derive(Debug, Default)]
struct Cache {
    clear_redelegation: bool,
    delegated: Option<Delegated>,
    delegate_start_slot: Option<DelegateStartSlot>,
    inflight_delegation: Option<InflightDelegation>,
    inflight_deposit: Option<InflightDeposit>,
    inflight_fee_payable: Option<InflightFeePayable>,
    inflight_rewards_receivable: Option<InflightRewardsReceivable>,
    inflight_unbond: Option<InflightUnbond>,
    last_reconcile_height: Option<LastReconcileHeight>,
    msg_issued_count: Option<MsgIssuedCount>,
    msg_success_count: Option<MsgSuccessCount>,
    pending_deposit: Option<PendingDeposit>,
    pending_unbond: Option<PendingUnbond>,
    undelegate_start_slot: Option<UndelegateStartSlot>,
    weights: Option<Weights>,
}

impl Cache {
    fn into_cmds(self) -> Vec<Cmd> {
        [
            self.clear_redelegation
                .then_some(Cmd::ClearRedelegationRequest),
            self.delegated.map(Cmd::from),
            self.delegate_start_slot.map(Cmd::from),
            self.inflight_delegation.map(Cmd::from),
            self.inflight_deposit.map(Cmd::from),
            self.inflight_fee_payable.map(Cmd::from),
            self.inflight_rewards_receivable.map(Cmd::from),
            self.inflight_unbond.map(Cmd::from),
            self.last_reconcile_height.map(Cmd::from),
            self.msg_issued_count.map(Cmd::from),
            self.msg_success_count.map(Cmd::from),
            self.pending_deposit.map(Cmd::from),
            self.pending_unbond.map(Cmd::from),
            self.undelegate_start_slot.map(Cmd::from),
            self.weights.map(Cmd::from),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

struct IntermediateRepo<'a> {
    repo: &'a dyn Repository,
    cache: Cache,
}

impl<'a> IntermediateRepo<'a> {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::ClearRedelegationRequest => self.cache.clear_redelegation = true,
            Cmd::DelegateStartSlot(v) => self.cache.delegate_start_slot = Some(v),
            Cmd::Delegated(v) => self.cache.delegated = Some(v),
            Cmd::InflightDelegation(v) => self.cache.inflight_delegation = Some(v),
            Cmd::InflightDeposit(v) => self.cache.inflight_deposit = Some(v),
            Cmd::InflightFeePayable(v) => self.cache.inflight_fee_payable = Some(v),
            Cmd::InflightRewardsReceivable(v) => self.cache.inflight_rewards_receivable = Some(v),
            Cmd::InflightUnbond(v) => self.cache.inflight_unbond = Some(v),
            Cmd::LastReconcileHeight(v) => self.cache.last_reconcile_height = Some(v),
            Cmd::MsgIssuedCount(v) => self.cache.msg_issued_count = Some(v),
            Cmd::MsgSuccessCount(v) => self.cache.msg_success_count = Some(v),
            Cmd::PendingDeposit(v) => self.cache.pending_deposit = Some(v),
            Cmd::PendingUnbond(v) => self.cache.pending_unbond = Some(v),
            Cmd::UndelegateStartSlot(v) => self.cache.undelegate_start_slot = Some(v),
            Cmd::Weights(v) => self.cache.weights = Some(v),
            _ => panic!("unexpected cmd: {cmd:?}"),
        }
    }
}

impl<'a> Repository for IntermediateRepo<'a> {
    fn delegated(&self) -> Delegated {
        self.cache
            .delegated
            .unwrap_or_else(|| self.repo.delegated())
    }

    fn delegate_start_slot(&self) -> DelegateStartSlot {
        self.cache
            .delegate_start_slot
            .unwrap_or_else(|| self.repo.delegate_start_slot())
    }

    fn inflight_delegation(&self) -> InflightDelegation {
        self.cache
            .inflight_delegation
            .unwrap_or_else(|| self.repo.inflight_delegation())
    }

    fn inflight_deposit(&self) -> InflightDeposit {
        self.cache
            .inflight_deposit
            .unwrap_or_else(|| self.repo.inflight_deposit())
    }

    fn inflight_fee_payable(&self) -> InflightFeePayable {
        self.cache
            .inflight_fee_payable
            .unwrap_or_else(|| self.repo.inflight_fee_payable())
    }

    fn inflight_rewards_receivable(&self) -> InflightRewardsReceivable {
        self.cache
            .inflight_rewards_receivable
            .unwrap_or_else(|| self.repo.inflight_rewards_receivable())
    }

    fn inflight_unbond(&self) -> InflightUnbond {
        self.cache
            .inflight_unbond
            .unwrap_or_else(|| self.repo.inflight_unbond())
    }

    fn last_reconcile_height(&self) -> Option<LastReconcileHeight> {
        self.cache
            .last_reconcile_height
            .or_else(|| self.repo.last_reconcile_height())
    }

    fn msg_issued_count(&self) -> MsgIssuedCount {
        self.cache
            .msg_issued_count
            .unwrap_or_else(|| self.repo.msg_issued_count())
    }

    fn msg_success_count(&self) -> MsgSuccessCount {
        self.cache
            .msg_success_count
            .unwrap_or_else(|| self.repo.msg_success_count())
    }

    fn pending_deposit(&self) -> PendingDeposit {
        self.cache
            .pending_deposit
            .unwrap_or_else(|| self.repo.pending_deposit())
    }

    fn pending_unbond(&self) -> PendingUnbond {
        self.cache
            .pending_unbond
            .unwrap_or_else(|| self.repo.pending_unbond())
    }

    fn phase(&self) -> Phase {
        self.repo.phase()
    }

    fn state(&self) -> State {
        self.repo.state()
    }

    fn redelegation_slot(&self) -> Option<RedelegationSlot> {
        if self.cache.clear_redelegation {
            return None;
        }

        self.repo.redelegation_slot()
    }

    fn undelegate_start_slot(&self) -> UndelegateStartSlot {
        self.cache
            .undelegate_start_slot
            .unwrap_or_else(|| self.repo.undelegate_start_slot())
    }

    fn weights(&self) -> Weights {
        self.cache
            .weights
            .clone()
            .unwrap_or_else(|| self.repo.weights())
    }
}

type Handler = fn(Context) -> Transition;

fn start_setup_rewards_address(Context { env, .. }: Context) -> Transition {
    let Some((delegation_account, rewards_account)) = env
        .delegation_account_address()
        .zip(env.rewards_account_address())
    else {
        // cannot continue until ICAs have been setup
        return Transition::abort();
    };

    let tx_msgs = TxMsgs::single(TxMsg::SetRewardsWithdrawalAddress(
        delegation_account,
        rewards_account,
    ));

    Transition::tx(tx_msgs, vec![])
}

fn on_setup_rewards_address_success(_: Context) -> Transition {
    Transition::next(vec![])
}

fn start_setup_authz(Context { env, .. }: Context) -> Transition {
    let (delegation_account, rewards_account) = env
        .delegation_account_address()
        .zip(env.rewards_account_address())
        .expect("always: there must be delegation and rewards addresses to access this phase");

    let tx_msgs = TxMsgs::single(TxMsg::GrantAuthzSend(rewards_account, delegation_account));

    Transition::tx(tx_msgs, vec![])
}

fn on_setup_authz_success(_: Context) -> Transition {
    Transition::next(vec![])
}

struct Slashing {
    /// The post-slashing weights
    adjusted_weights: Weights,
    /// The post-slashing delegated amount
    delegated: u128,
    /// The post-slashing pending unbond amount
    pending_unbond: u128,
    /// The post-slashing inflight unbond amount
    inflight_unbond: u128,
    /// The ratio of post-slashed-delegations / pre-slashed-delegations
    slashed_ratio: FixedU256,
}

// determine whether a slashing has occured
fn check_for_slashing(
    Delegated(delegated): Delegated,
    PendingUnbond(pending_unbond): PendingUnbond,
    InflightUnbond(inflight_unbond): InflightUnbond,
    LastReconcileHeight(last_reconcile_height): LastReconcileHeight,
    delegations: DelegationsReport,
) -> Option<Slashing> {
    if delegated == 0 {
        return None;
    }

    // only continue if the delegations report is not stale
    if delegations.height <= last_reconcile_height {
        return None;
    }

    // only continue if there is a loss
    if delegations.total_delegated >= delegated {
        return None;
    }

    let mut adjusted_weights = Vec::with_capacity(delegations.delegated_amounts_per_slot.len());

    for delegation in delegations.delegated_amounts_per_slot {
        let adjusted_weight =
            Weight::checked_from_fraction(delegation, delegations.total_delegated)
                .expect("always: delegation <= total delegation");

        adjusted_weights.push(adjusted_weight);
    }

    let adjusted_weights =
        Weights::new(&adjusted_weights).expect("always: one weight per slot & total weight == 1.0");

    let slashed_ratio = FixedU256::from_u128(delegations.total_delegated)
        .checked_div(FixedU256::from_u128(delegated))
        .expect("checked: delegated > 0");

    let pending_unbond = slashed_ratio
        .checked_mul(FixedU256::from_u128(pending_unbond))
        .expect("always: slashed ratio < 1.0")
        .floor();

    let inflight_unbond = slashed_ratio
        .checked_mul(FixedU256::from_u128(inflight_unbond))
        .expect("always: slashed ratio < 1.0")
        .floor();

    Some(Slashing {
        adjusted_weights,
        delegated: delegations.total_delegated,
        pending_unbond,
        inflight_unbond,
        slashed_ratio,
    })
}

fn start_reconcile(Context { repo, env, .. }: Context) -> Transition {
    let Some(last_reconcile_height) = repo.last_reconcile_height() else {
        return Transition::next(vec![]);
    };

    let Some(delegations) = env.delegations_report() else {
        return Transition::next(vec![]);
    };

    let Some(slashing) = check_for_slashing(
        repo.delegated(),
        repo.pending_unbond(),
        repo.inflight_unbond(),
        last_reconcile_height,
        delegations,
    ) else {
        return Transition::next(vec![]);
    };

    let mut cmds = set![slashing.adjusted_weights, Delegated(slashing.delegated)];

    if slashing.pending_unbond > 0 {
        cmds.push(PendingUnbond(slashing.pending_unbond).into());
    }

    if slashing.inflight_unbond > 0 {
        cmds.push(InflightUnbond(slashing.inflight_unbond).into());
    }

    Transition::next(cmds).event(Event::SlashDetected(slashing.slashed_ratio))
}

fn start_redelegate(Context { repo, env, .. }: Context) -> Transition {
    let Some(LastReconcileHeight(last_reconcile_height)) = repo.last_reconcile_height() else {
        return Transition::next(vec![]);
    };

    let Some(RedelegationSlot(ValidatorSetSlot(slot))) = repo.redelegation_slot() else {
        return Transition::next(vec![]);
    };

    let Some(delegations) = env.delegations_report() else {
        return Transition::next(vec![]);
    };

    // we can only use delegations after the previous reconciliation
    if delegations.height <= last_reconcile_height {
        return Transition::next(vec![]);
    }

    let delegated_amount = delegations
        .delegated_amounts_per_slot
        .get(slot)
        .expect("valid slot index");

    let tx_msgs = TxMsgs::single(TxMsg::Redelegate(ValidatorSetSlot(slot), *delegated_amount));

    Transition::tx(tx_msgs, vec![])
}

fn on_redelegate_success(_: Context) -> Transition {
    Transition::next(set![Cmd::ClearRedelegationRequest]).event(Event::RedelegationSuccessful)
}

// Do not retry on failure, clear request and move on
fn on_redelegate_failure(_: Context) -> Transition {
    Transition::next(set![Cmd::ClearRedelegationRequest])
}

type Undelegation = (ValidatorSetSlot, NonZeroU128);

// Normalize the weights so they add up to 1.0 if they do not already.
// If all the given weights are 0, the scaled weights will all be: 1.0 / n weights.
// Panics if the `weights` add up to > 1.0
fn normalize_weights(weights: &[Weight]) -> Option<Weights> {
    if weights.is_empty() {
        return None;
    }

    let total_weight = weights
        .iter()
        .copied()
        .map(Weight::into_fixed)
        .reduce(|acc, w| {
            acc.checked_add(w)
                .expect("always: total weights should never overflow Q128.128")
        })
        .expect("checked: weights len > 0");

    let one = FixedU256::from_u128(1);

    assert!(
        total_weight <= one,
        "cannot scale weights that already add up to > 1.0"
    );

    if total_weight == one {
        return Some(Weights::new_unchecked(weights.to_vec()));
    }

    if total_weight.is_zero() {
        let equal_weight = FixedU256::from_u128(1)
            .checked_div(FixedU256::from_u128(weights.len() as u128))
            .map(Weight::checked_from_fixed)
            .expect("checked: weights len > 0")
            .expect("always: 1 divided by non zero <= 1.0");

        let equal_weights = vec![equal_weight; weights.len()];

        return Some(Weights::new_unchecked(equal_weights));
    }

    let scaled_weights = weights
        .iter()
        .copied()
        .map(Weight::into_fixed)
        .map(|w| {
            w.checked_div(total_weight)
                .map(Weight::checked_from_fixed)
                .expect("checked: weights len > 0")
                .expect("always: w <= total weight")
        })
        .collect();

    Some(Weights::new_unchecked(scaled_weights))
}

fn distribute_undelegations(
    weights: &[Weight],
    Delegated(delegated): Delegated,
    unbond_amount: u128,
    slot_offset: usize,
) -> impl Iterator<Item = Undelegation> + '_ {
    assert!(!weights.is_empty(), "cannot undelegate from 0 slots");

    normalize_weights(weights)
        .expect("checked: weights len > 0")
        .into_iter()
        .zip(weights)
        .map(move |(scaled_w, original_w)| {
            // take the minimum of the total delegated amount to a slot and the scaled allocated unbond amount
            original_w
                .apply(delegated)
                .min(scaled_w.apply(unbond_amount))
        })
        .enumerate()
        // skip slots where the split amount is zero
        .filter_map(move |(idx, amount)| {
            Some(ValidatorSetSlot(idx + slot_offset)).zip(NonZeroU128::new(amount))
        })
}

fn undelegate_tx_msgs(
    config: &dyn Config,
    repo: &dyn Repository,
    unbond_amount: u128,
) -> Option<TxMsgs> {
    let UndelegateStartSlot(start_slot_idx) = repo.undelegate_start_slot();

    let weights = repo.weights();

    if start_slot_idx == 0 {
        // no need to take a subset of slot weights
        let undelegate_msgs = distribute_undelegations(
            weights.as_slice(),
            repo.delegated(),
            unbond_amount,
            start_slot_idx,
        )
        .map(|(slot, amount)| TxMsg::Undelegate(slot, amount.get()));

        return TxMsgBatcher::new(config, repo).batch_msgs(undelegate_msgs);
    }

    let weights = &weights.as_slice()[start_slot_idx..];

    let undelegate_msgs =
        distribute_undelegations(weights, repo.delegated(), unbond_amount, start_slot_idx)
            .map(|(slot, amount)| TxMsg::Undelegate(slot, amount.get()));

    TxMsgBatcher::new(config, repo).batch_msgs(undelegate_msgs)
}

fn undelegate_adjust_weights(
    weights: &Weights,
    previous_delegated: u128,
    current_delegated: u128,
    undelegations: impl Iterator<Item = Undelegation>,
) -> Option<Weights> {
    if current_delegated == 0 {
        return None;
    }

    let undelegations: BTreeMap<_, _> = undelegations
        .map(|(ValidatorSetSlot(slot), amount)| (slot, amount.get()))
        .collect();

    let mut adjusted_weights = vec![];

    for (slot, weight) in weights.as_slice().iter().enumerate() {
        let mut slot_delegation = weight.apply(previous_delegated);

        if let Some(undelegation) = undelegations.get(&slot) {
            slot_delegation = slot_delegation
                .checked_sub(*undelegation)
                .expect("always: previous slot delegation >= slot undelegation");
        }

        let adjusted_w = Weight::checked_from_fraction(slot_delegation, current_delegated)
            .expect("checked: current delegation > 0");

        adjusted_weights.push(adjusted_w);
    }

    Weights::new(&adjusted_weights)
}

fn start_undelegate(Context { repo, config, .. }: Context) -> Transition {
    let PendingUnbond(pending_unbond) = repo.pending_unbond();
    let Delegated(delegated) = repo.delegated();

    // The pending unbond amount can be greater than the delegated amount if pending deposits
    // are being unbonded before they had chance to be delegated.
    //
    // Only undelegating if pending unbond <= delegated is a simple way to ensure ordering.
    if pending_unbond > delegated || pending_unbond == 0 {
        return Transition::next(vec![]);
    }

    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();

    // If there is any leftover inflight unbond, clear that before taking on more pending unbonds
    let unbond_amount = if inflight_unbond > 0 {
        inflight_unbond
    } else {
        pending_unbond
    };

    let Some(tx_msgs) = undelegate_tx_msgs(config, repo, unbond_amount) else {
        return Transition::next(vec![]);
    };

    let mut cmds = vec![];

    if inflight_unbond == 0 {
        cmds.push(InflightUnbond(unbond_amount).into());
    }

    Transition::tx(tx_msgs, cmds)
}

fn on_undelegate_success(Context { repo, config, .. }: Context) -> Transition {
    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();

    if let Some(tx_msgs) = undelegate_tx_msgs(config, repo, inflight_unbond) {
        return Transition::tx(tx_msgs, vec![]);
    };

    let PendingUnbond(pending_unbond) = repo.pending_unbond();

    let Delegated(prev_delegated) = repo.delegated();

    let delegated = prev_delegated - inflight_unbond;

    let mut cmds = set![
        PendingUnbond(pending_unbond - inflight_unbond),
        InflightUnbond(0),
        Delegated(delegated)
    ];

    let UndelegateStartSlot(start_slot_idx) = repo.undelegate_start_slot();

    if start_slot_idx > 0 {
        // reset starting slot to the first one
        cmds.push(UndelegateStartSlot(0).into());

        let weights = repo.weights();

        let undelegations = distribute_undelegations(
            &weights.as_slice()[start_slot_idx..],
            Delegated(prev_delegated),
            inflight_unbond,
            start_slot_idx,
        );

        let adjusted_weights =
            undelegate_adjust_weights(&weights, prev_delegated, delegated, undelegations)
                .unwrap_or_else(|| config.starting_weights());

        cmds.push(adjusted_weights.into())
    }

    Transition::next(cmds).event(Event::UnbondStarted(inflight_unbond))
}

fn retry_undelegate(Context { repo, config, .. }: Context) -> Transition {
    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();

    let tx_msgs = undelegate_tx_msgs(config, repo, inflight_unbond)
        .expect("always: messages to re-issue when retrying");

    Transition::tx(tx_msgs, vec![])
}

fn undelegate_force_next(Context { repo, config, .. }: Context) -> (Vec<Event>, Vec<Cmd>) {
    let MsgSuccessCount(msg_success_count) = repo.msg_success_count();
    let UndelegateStartSlot(start_slot_idx) = repo.undelegate_start_slot();

    if msg_success_count == 0 {
        // was the last undelegation partial? i.e. forced next after a successful batch
        if start_slot_idx > 0 {
            // no change, try again next time
            return (vec![], vec![]);
        }

        // non-partial undelegation, clear inflight unbond
        return (vec![], set![InflightUnbond(0)]);
    }

    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();
    let PendingUnbond(pending_unbond) = repo.pending_unbond();
    let Delegated(prev_delegated) = repo.delegated();

    let weights = repo.weights();

    let undelegations: Vec<_> = distribute_undelegations(
        weights.as_slice(),
        Delegated(prev_delegated),
        inflight_unbond,
        start_slot_idx,
    )
    .take(msg_success_count)
    .collect();

    let total_unbonded: u128 = undelegations
        .iter()
        .take(msg_success_count)
        .try_fold(0u128, |sum, (_, amount)| sum.checked_add(amount.get()))
        .expect("always: inflight unbond <= delegated <= u128::MAX");

    let delegated = prev_delegated
        .checked_sub(total_unbonded)
        .expect("always: total unbonded <= inflight unbond <= delegated");

    // Undelegation should start at the slot after the last successful undelegation
    let undelegate_start_slot = undelegations
        .last()
        .map(|(ValidatorSetSlot(slot), _)| slot + 1)
        .expect("always: undelegations length > 0 when msg success count > 0");

    let adjusted_weights = undelegate_adjust_weights(
        &weights,
        prev_delegated,
        delegated,
        undelegations.into_iter(),
    )
    .unwrap_or_else(|| config.starting_weights());

    let cmds = set![
        Delegated(delegated),
        PendingUnbond(pending_unbond - total_unbonded),
        InflightUnbond(inflight_unbond - total_unbonded),
        UndelegateStartSlot(undelegate_start_slot),
        adjusted_weights
    ];

    let events = vec![Event::UnbondStarted(total_unbonded)];

    (events, cmds)
}

fn start_transfer_undelegated(Context { repo, env, .. }: Context) -> Transition {
    let Some(LastReconcileHeight(last_reconcile_height)) = repo.last_reconcile_height() else {
        return Transition::next(vec![]);
    };

    let Some(UndelegatedBalanceReport { remote_balance, .. }) = env.undelegated_balance_report()
    else {
        return Transition::next(vec![]);
    };

    if remote_balance.height <= last_reconcile_height {
        return Transition::next(vec![]);
    }

    let RemoteBalance(amount) = remote_balance.amount;

    if amount == 0 {
        return Transition::next(vec![]);
    };

    let tx_msgs = TxMsgs::single(TxMsg::TransferInUndelegated(amount));

    Transition::tx(tx_msgs, vec![])
}

fn on_transfer_undelegated_success(_: Context) -> Transition {
    Transition::next(vec![]).event(Event::UndelegatedAssetsTransferred)
}

fn start_transfer_pending_deposits(Context { repo, .. }: Context) -> Transition {
    let PendingDeposit(pending_deposit) = repo.pending_deposit();
    let InflightDeposit(inflight_deposit) = repo.inflight_deposit();

    // Nothing to do if there are no pending deposits or if there are still inflight deposits to clear
    if pending_deposit == 0 || inflight_deposit > 0 {
        return Transition::next(vec![]);
    }

    let tx_msgs = TxMsgs::single(TxMsg::TransferOutPendingDeposit(pending_deposit));

    let cmds = set![InflightDeposit(pending_deposit)];

    Transition::tx(tx_msgs, cmds)
}

fn on_transfer_pending_deposits_success(Context { repo, .. }: Context) -> Transition {
    let PendingDeposit(pending_deposit) = repo.pending_deposit();

    let InflightDeposit(inflight_deposit) = repo.inflight_deposit();

    Transition::next(set![PendingDeposit(pending_deposit - inflight_deposit)])
        .event(Event::DepositsTransferred(inflight_deposit))
}

#[derive(Debug, Clone, Copy)]
struct DelegatePhaseBalances {
    delegation: InflightDelegation,
    rewards_receivable: InflightRewardsReceivable,
    fee_payable: InflightFeePayable,
}

fn delegate_phase_balances(
    config: &dyn Config,
    repo: &dyn Repository,
    env: &dyn Env,
) -> Option<DelegatePhaseBalances> {
    let InflightDelegation(inflight_delegation) = repo.inflight_delegation();

    if inflight_delegation != 0 {
        return Some(DelegatePhaseBalances {
            delegation: InflightDelegation(inflight_delegation),
            rewards_receivable: repo.inflight_rewards_receivable(),
            fee_payable: repo.inflight_fee_payable(),
        });
    }

    let delegate_deposits_only = || {
        let InflightDeposit(inflight_deposit) = repo.inflight_deposit();

        if inflight_deposit == 0 {
            return None;
        }

        Some(DelegatePhaseBalances {
            delegation: InflightDelegation(inflight_deposit),
            rewards_receivable: InflightRewardsReceivable(0),
            fee_payable: InflightFeePayable(0),
        })
    };

    let DelegateStartSlot(start_slot) = repo.delegate_start_slot();

    // was the previous delegation a partial one?
    if start_slot > 0 {
        // delegations should be comprised of previously moved rewards and inflight deposits
        let InflightDeposit(inflight_deposit) = repo.inflight_deposit();
        let InflightRewardsReceivable(rewards) = repo.inflight_rewards_receivable();

        let inflight_delegation = inflight_deposit
            .checked_add(rewards)
            .expect("always: this inflight delegation < previous inflight delegation");

        assert!(
            inflight_delegation > 0,
            "inflight delegation > 0 when the previous delegation was partial"
        );

        return Some(DelegatePhaseBalances {
            delegation: InflightDelegation(inflight_delegation),
            rewards_receivable: InflightRewardsReceivable(rewards),
            fee_payable: InflightFeePayable(0),
        });
    }

    let Some(last_reconcile_height) = repo.last_reconcile_height() else {
        return delegate_deposits_only();
    };

    let Some(total_rewards) = env.rewards_balance(last_reconcile_height) else {
        return delegate_deposits_only();
    };

    let InflightDeposit(inflight_deposit) = repo.inflight_deposit();

    let Some(fee) = fee_metadata(config, env).fee_bps(last_reconcile_height, env.current_height())
    else {
        return Some(DelegatePhaseBalances {
            delegation: InflightDelegation(inflight_deposit + total_rewards.get()),
            rewards_receivable: InflightRewardsReceivable(total_rewards.get()),
            fee_payable: InflightFeePayable(0),
        });
    };

    let (rewards_receivable, fee_payable) = fee.apply_to(total_rewards);

    Some(DelegatePhaseBalances {
        delegation: InflightDelegation(inflight_deposit + rewards_receivable.get()),
        rewards_receivable: InflightRewardsReceivable(rewards_receivable.get()),
        fee_payable: fee_payable.map_or(InflightFeePayable(0), |fee| InflightFeePayable(fee.get())),
    })
}

// Invert the weights and then normalize to 1.0
// `[ 0.4 0.4 0.1 0.1 ]` would become `[ 0.1 0.1 0.4 0.4 ]`
fn rebalance_weights(weights: Weights) -> Weights {
    let mut total_inverted_weight = FixedU256::zero();

    let mut inverted_weights = Vec::with_capacity(weights.as_slice().len());

    let one = FixedU256::from_u128(1);

    for weight in weights.into_iter().map(Weight::into_fixed) {
        if weight.is_zero() {
            inverted_weights.push(FixedU256::zero());
            continue;
        }

        let inverted_weight = one.checked_div(weight).expect("checked: weight > 0");

        total_inverted_weight = total_inverted_weight
            .checked_add(inverted_weight)
            .expect("always: total inverted weights should never overflow Q128.128");

        inverted_weights.push(inverted_weight);
    }

    let rebalance_weights = inverted_weights
        .into_iter()
        .map(|iw| {
            iw.checked_div(total_inverted_weight)
                .map(Weight::checked_from_fixed)
                .expect("always: total inverted weights > 0 if total weights > 0")
                .expect("always: 1 divided by non zero <= 1.0")
        })
        .collect();

    Weights::new_unchecked(rebalance_weights)
}

type Delegation = (ValidatorSetSlot, NonZeroU128);

// distribute delegations so that the weights trend towards equalisation, i.e. lower weighted slots receive more
fn distribute_delegations(
    weights: &[Weight],
    total_delegation: u128,
    slot_offset: usize,
) -> impl Iterator<Item = Delegation> {
    assert!(
        !weights.is_empty(),
        "cannot distribute delegations across 0 slots"
    );

    let scaled_weights = normalize_weights(weights).expect("checked: weights len > 0");

    let rebalance_weights = rebalance_weights(scaled_weights);

    let mut total_allocated = 0u128;
    let mut delegations = Vec::with_capacity(weights.len());

    for weight in rebalance_weights.as_slice() {
        let delegation = weight.apply(total_delegation);

        total_allocated = total_allocated
            .checked_add(delegation)
            .expect("always: total allocated <= total delegation");

        delegations.push(delegation);
    }

    let unallocated = total_allocated.abs_diff(total_delegation);

    // assign any unallocated delegation to the lowest weighted slot
    let (lowest_weight_slot_idx, _) = weights
        .iter()
        .enumerate()
        .min_by_key(|(_, w)| w.into_fixed())
        .expect("checked: weights len > 0");

    delegations[lowest_weight_slot_idx] = delegations[lowest_weight_slot_idx]
        .checked_add(unallocated)
        .expect("always: any slot allocation + unallocated <= total delegation");

    delegations
        .into_iter()
        // get the indexes of the slots
        .enumerate()
        // skip slots where the split amount is zero
        .filter_map(move |(idx, amount)| {
            NonZeroU128::new(amount).map(|amount| (ValidatorSetSlot(idx + slot_offset), amount))
        })
}

fn delegate_phase_msgs(
    weights: &[Weight],
    balances: DelegatePhaseBalances,
    slot_offset: usize,
    fee_recipient: Option<FeeRecipient>,
) -> impl Iterator<Item = TxMsg> {
    let InflightDelegation(inflight_delegation) = balances.delegation;

    let delegate_msgs = distribute_delegations(weights, inflight_delegation, slot_offset)
        // create undelegate msg
        .map(|(slot, amount)| TxMsg::Delegate(slot, amount.get()));

    let InflightRewardsReceivable(rewards_receivable) = balances.rewards_receivable;

    let send_rewards_receivable: Option<TxMsg> = (rewards_receivable != 0)
        .then_some(RewardsReceivable(rewards_receivable))
        .map(AuthzMsg::SendRewardsReceivable)
        .map(|auth_z_msg| TxMsg::Authz(vec![auth_z_msg]));

    let InflightFeePayable(fee_payable) = balances.fee_payable;

    let send_fee_msg: Option<TxMsg> = (fee_payable != 0)
        .then_some(ReconcilerFee(fee_payable))
        .zip(fee_recipient)
        .map(|(fee, recipient)| AuthzMsg::SendFee(recipient, fee))
        .map(|auth_z_msg| TxMsg::Authz(vec![auth_z_msg]));

    send_rewards_receivable
        .into_iter()
        .chain(delegate_msgs)
        .chain(send_fee_msg)
}

fn delegate_tx_msgs(
    config: &dyn Config,
    repo: &dyn Repository,
    env: &dyn Env,
    balances: DelegatePhaseBalances,
) -> Option<TxMsgs> {
    let DelegateStartSlot(start_slot_idx) = repo.delegate_start_slot();

    let weights = repo.weights();

    if start_slot_idx == 0 {
        // no need to take a subset of slot weights
        let msgs = delegate_phase_msgs(
            weights.as_slice(),
            balances,
            start_slot_idx,
            env.fee_recipient(),
        );

        return TxMsgBatcher::new(config, repo).batch_msgs(msgs);
    }

    // take a subset of the slots starting at the start slot index set in a previous round
    let weights = &weights.as_slice()[start_slot_idx..];

    let msgs = delegate_phase_msgs(weights, balances, start_slot_idx, env.fee_recipient());

    TxMsgBatcher::new(config, repo).batch_msgs(msgs)
}

fn delegate_adjust_weights(
    weights: &Weights,
    previous_delegated: u128,
    current_delegated: u128,
    delegations: impl Iterator<Item = Delegation>,
) -> Weights {
    let delegations: BTreeMap<_, _> = delegations
        .map(|(ValidatorSetSlot(slot), amount)| (slot, amount.get()))
        .collect();

    let mut adjusted_weights = vec![];

    for (slot, weight) in weights.as_slice().iter().enumerate() {
        let mut slot_delegation = weight.apply(previous_delegated);

        if let Some(delegation) = delegations.get(&slot) {
            slot_delegation = slot_delegation.checked_add(*delegation).expect(
                "always: delegation would have failed if new delegated amount overflowed 128 bits",
            );
        }

        let adjusted_w = Weight::checked_from_fraction(slot_delegation, current_delegated)
            .expect("checked: current delegation > prev delegation >= 0");

        adjusted_weights.push(adjusted_w);
    }

    Weights::new(&adjusted_weights).expect("always: valid weights")
}

fn try_withdraw_rewards(config: &dyn Config, repo: &dyn Repository) -> Transition {
    let Delegated(delegated) = repo.delegated();

    if repo.last_reconcile_height().is_none() || delegated == 0 {
        return Transition::next(vec![]);
    }

    let ValidatorSetSize(validator_set_size) = config.validator_set_size();

    let msgs = (0..validator_set_size)
        .map(ValidatorSetSlot)
        .map(TxMsg::WithdrawRewards);

    let Some(tx_msgs) = TxMsgBatcher::new(config, repo).batch_msgs(msgs) else {
        return Transition::next(vec![]);
    };

    Transition::tx(tx_msgs, vec![])
}

fn start_delegate(Context { config, repo, env }: Context) -> Transition {
    let Some(delegate_balances) = delegate_phase_balances(config, repo, env) else {
        return try_withdraw_rewards(config, repo);
    };

    let tx_msgs = delegate_tx_msgs(config, repo, env, delegate_balances)
        .expect("always: at least one message if there are delegatable balances");

    let mut cmds = vec![];

    if delegate_balances.delegation.0 != 0 {
        cmds.push(delegate_balances.delegation.into());
    }

    if delegate_balances.rewards_receivable.0 != 0 {
        cmds.push(delegate_balances.rewards_receivable.into());
    }

    if delegate_balances.fee_payable.0 != 0 {
        cmds.push(delegate_balances.fee_payable.into());
    }

    Transition::tx(tx_msgs, cmds)
}

fn on_delegate_success(Context { config, repo, env }: Context) -> Transition {
    let Some(delegate_balances) = delegate_phase_balances(config, repo, env) else {
        return try_withdraw_rewards(config, repo);
    };

    if let Some(tx_msgs) = delegate_tx_msgs(config, repo, env, delegate_balances) {
        return Transition::tx(tx_msgs, vec![]);
    }

    let Delegated(prev_delegated) = repo.delegated();

    let InflightDelegation(inflight_delegation) = delegate_balances.delegation;

    let delegated = prev_delegated
        .checked_add(inflight_delegation)
        .expect("always: total delegated amount should never overflow u128");

    let weights = repo.weights();

    let DelegateStartSlot(start_slot) = repo.delegate_start_slot();

    let delegations = distribute_delegations(
        &weights.as_slice()[start_slot..],
        inflight_delegation,
        start_slot,
    );

    let adjusted_weights =
        delegate_adjust_weights(&weights, prev_delegated, delegated, delegations);

    let cmds = set![
        Delegated(delegated),
        InflightDelegation(0),
        InflightDeposit(0),
        InflightRewardsReceivable(0),
        InflightFeePayable(0),
        DelegateStartSlot(0),
        adjusted_weights
    ];

    Transition::next(cmds).event(Event::DelegationsIncreased(inflight_delegation))
}

fn delegate_force_next(Context { repo, .. }: Context) -> (Vec<Event>, Vec<Cmd>) {
    let MsgSuccessCount(msg_success_count) = repo.msg_success_count();
    let DelegateStartSlot(start_slot_idx) = repo.delegate_start_slot();

    if msg_success_count == 0 {
        // was the last delegation partial? i.e. forced next after a successful batch
        if start_slot_idx > 0 {
            // no change, try again next time
            return (vec![], vec![]);
        }

        // no batches ever succeeded: clear inflight balances, try again next time as normal
        return (
            vec![],
            set![
                InflightDelegation(0),
                InflightDeposit(0),
                InflightRewardsReceivable(0),
                InflightFeePayable(0)
            ],
        );
    }

    let InflightDelegation(inflight_delegation) = repo.inflight_delegation();
    let Delegated(prev_delegated) = repo.delegated();
    let InflightRewardsReceivable(rewards) = repo.inflight_rewards_receivable();

    let weights = repo.weights();

    let delegate_msg_success_count = if start_slot_idx == 0 && rewards > 0 {
        // The first message is not a delegate msgs, but an authz bank send
        msg_success_count - 1
    } else {
        msg_success_count
    };

    let delegations: Vec<_> =
        distribute_delegations(weights.as_slice(), inflight_delegation, start_slot_idx)
            .take(delegate_msg_success_count)
            .collect();

    let successfully_delegated: u128 = delegations
        .iter()
        .try_fold(0u128, |sum, (_, amount)| sum.checked_add(amount.get()))
        .expect("always: successfully delegated < inflight delegation");

    let delegated = prev_delegated
        .checked_add(successfully_delegated)
        .expect("always: total delegated amount should never overflow u128");

    // The delegation should recommence at the slot after the last successful undelegation
    let delegate_start_slot = delegations
        .last()
        .map(|(ValidatorSetSlot(slot), _)| slot + 1)
        .expect("always: delegations length > 0 when msg success count > 0");

    let adjusted_weights =
        delegate_adjust_weights(&weights, prev_delegated, delegated, delegations.into_iter());

    let InflightDeposit(inflight_deposit) = repo.inflight_deposit();

    let deposits_delegated = successfully_delegated.abs_diff(rewards);

    // draw down rewards first
    let rewards = rewards.saturating_sub(successfully_delegated);

    let inflight_deposit = inflight_deposit
        .checked_sub(deposits_delegated)
        .expect("always: inflight deposit == (inflight delegation - rewards)");

    let cmds = set![
        Delegated(delegated),
        // Clear inflight delegation so it is recalculated on the next pass
        InflightDelegation(0),
        InflightDeposit(inflight_deposit),
        InflightRewardsReceivable(rewards),
        // Discard any fee payment
        InflightFeePayable(0),
        DelegateStartSlot(delegate_start_slot),
        adjusted_weights
    ];

    let events = vec![Event::DelegationsIncreased(successfully_delegated)];

    (events, cmds)
}

const fn handler(phase: Phase, state: State) -> Handler {
    match (phase, state) {
        (Phase::SetupRewardsAddress, State::Idle | State::Failed) => start_setup_rewards_address,
        (Phase::SetupRewardsAddress, State::Pending) => on_setup_rewards_address_success,
        (Phase::SetupAuthz, State::Idle | State::Failed) => start_setup_authz,
        (Phase::SetupAuthz, State::Pending) => on_setup_authz_success,
        (Phase::StartReconcile, _) => start_reconcile,
        (Phase::Redelegate, State::Idle) => start_redelegate,
        (Phase::Redelegate, State::Pending) => on_redelegate_success,
        (Phase::Redelegate, State::Failed) => on_redelegate_failure,
        (Phase::Undelegate, State::Idle) => start_undelegate,
        (Phase::Undelegate, State::Pending) => on_undelegate_success,
        (Phase::Undelegate, State::Failed) => retry_undelegate,
        (Phase::TransferUndelegated, State::Idle | State::Failed) => start_transfer_undelegated,
        (Phase::TransferUndelegated, State::Pending) => on_transfer_undelegated_success,
        (Phase::TransferPendingDeposits, State::Idle | State::Failed) => {
            start_transfer_pending_deposits
        }
        (Phase::TransferPendingDeposits, State::Pending) => on_transfer_pending_deposits_success,
        (Phase::Delegate, State::Idle | State::Failed) => start_delegate,
        (Phase::Delegate, State::Pending) => on_delegate_success,
    }
}

fn reconcile(
    ctx: Context,
    mut phase: Phase,
    mut state: State,
    mut intermediate_repo: IntermediateRepo,
    mut all_events: Vec<Event>,
) -> Response {
    let mut tx_skip_count = 0;

    loop {
        let Transition { kind, cmds, events } = handler(phase, state)(Context {
            repo: &intermediate_repo,
            ..ctx
        });

        all_events.extend_from_slice(&events);

        for cmd in cmds {
            intermediate_repo.handle_cmd(cmd);
        }

        match kind {
            TransitionKind::Next => {
                intermediate_repo.handle_cmd(MsgIssuedCount(0).into());
                intermediate_repo.handle_cmd(MsgSuccessCount(0).into());

                // Possible Txs skipped
                if state.is_idle() {
                    let phase_tx_count =
                        phase.tx_count(ctx.config.validator_set_size(), ctx.config.max_msg_count());

                    tx_skip_count += phase_tx_count;
                }

                if let Some(next_phase) = phase.next() {
                    phase = next_phase;
                    state = State::Idle;
                    continue;
                }

                let mut cmds = intermediate_repo.cache.into_cmds();

                let CurrentHeight(current_height) = ctx.env.current_height();

                cmds.push(LastReconcileHeight(current_height).into());
                cmds.push(Phase::StartReconcile.into());
                cmds.push(State::Idle.into());

                return Response {
                    cmds,
                    events: all_events,
                    tx_msgs: None,
                    tx_skip_count,
                };
            }

            TransitionKind::Tx(tx_msgs) => {
                intermediate_repo.handle_cmd(MsgIssuedCount(tx_msgs.msgs.len()).into());

                let mut cmds = intermediate_repo.cache.into_cmds();

                cmds.push(phase.into());
                cmds.push(State::Pending.into());

                return Response {
                    cmds,
                    events: all_events,
                    tx_msgs: Some(tx_msgs),
                    tx_skip_count,
                };
            }

            TransitionKind::Abort => {
                let tx_skip_count = phase.sequence_tx_count(
                    state,
                    ctx.config.validator_set_size(),
                    ctx.config.max_msg_count(),
                );

                return Response {
                    cmds: vec![],
                    events: all_events,
                    tx_msgs: None,
                    tx_skip_count,
                };
            }
        }
    }
}

impl<'a> Fsm for FsmImpl<'a> {
    fn reconcile(&self) -> Response {
        let state = self.ctx.repo.state();

        let mut intermediate_repo = IntermediateRepo {
            repo: self.ctx.repo,
            cache: Cache::default(),
        };

        // issued messages were successful, adjust counts accordingly
        if state.is_pending() {
            let MsgIssuedCount(msg_issued_count) = intermediate_repo.msg_issued_count();
            let MsgSuccessCount(msg_success_count) = intermediate_repo.msg_success_count();

            intermediate_repo
                .handle_cmd(MsgSuccessCount(msg_success_count + msg_issued_count).into());
        }

        reconcile(
            self.ctx,
            self.ctx.repo.phase(),
            state,
            intermediate_repo,
            vec![],
        )
    }

    fn failed(&self) -> Response {
        if !self.ctx.repo.state().is_pending() {
            panic!("failed called in a non-pending state")
        }

        let cmds = set![State::Failed, MsgIssuedCount(0)];

        Response {
            cmds,
            events: vec![],
            tx_msgs: None,
            tx_skip_count: 0,
        }
    }

    fn force_next(&self) -> Option<Response> {
        let phase = self.ctx.repo.phase();

        if !self.ctx.repo.state().is_failed() {
            return None;
        }

        let (events, mut cmds) = match phase {
            Phase::Undelegate => undelegate_force_next(self.ctx),
            Phase::Delegate => delegate_force_next(self.ctx),
            _ => return None,
        };

        // ensure message counts are cleared
        cmds.push(MsgIssuedCount(0).into());
        cmds.push(MsgSuccessCount(0).into());

        let Some(next_phase) = phase.next() else {
            let CurrentHeight(current_height) = self.ctx.env.current_height();

            cmds.push(LastReconcileHeight(current_height).into());
            cmds.push(Phase::StartReconcile.into());
            cmds.push(State::Idle.into());

            return Some(Response {
                cmds,
                events,
                tx_msgs: None,
                tx_skip_count: 0,
            });
        };

        let mut intermediate_repo = IntermediateRepo {
            repo: self.ctx.repo,
            cache: Cache::default(),
        };

        for cmd in cmds {
            intermediate_repo.handle_cmd(cmd);
        }

        Some(reconcile(
            self.ctx,
            next_phase,
            State::Idle,
            intermediate_repo,
            events,
        ))
    }
}

#[cfg(test)]
mod test;
