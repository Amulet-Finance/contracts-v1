pub mod types;

use std::num::NonZeroU128;

use types::{
    Account, CurrentHeight, Delegated, DelegationsReport, FeeBpsBlockIncrement, FeeMetadata,
    FeePaymentCooldownBlocks, FeeRecipient, InflightDelegation, InflightDeposit,
    InflightFeePayable, InflightRewardsReceivable, InflightUnbond, LastReconcileHeight, MaxFeeBps,
    MaxMsgCount, MsgIssuedCount, MsgSuccessCount, Now, PendingDeposit, PendingUnbond, Phase,
    ReconcilerFee, RedelegationSlot, RemoteBalance, RemoteBalanceReport, RewardsReceivable, State,
    UnbondingTimeSecs, UndelegatedBalanceReport, ValidatorSetSize, ValidatorSetSlot, Weight,
    Weights,
};

/// Access fixed config
pub trait Config {
    fn unbonding_time(&self) -> UnbondingTimeSecs;

    fn max_msg_count(&self) -> MaxMsgCount;

    fn fee_payout_cooldown(&self) -> FeePaymentCooldownBlocks;

    fn fee_bps_block_increment(&self) -> FeeBpsBlockIncrement;

    fn max_fee_bps(&self) -> MaxFeeBps;

    fn validator_set_size(&self) -> ValidatorSetSize;
}

/// Access mutable storage
pub trait Repository {
    fn delegated(&self) -> Delegated;

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

    pub fn one(msg: TxMsg) -> Self {
        Self { msgs: vec![msg] }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum Cmd {
    ClearRedelegationRequest,
    Delegated(Delegated),
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
    Phase,
    State,
    PendingUnbond,
    PendingDeposit,
    Delegated,
    MsgIssuedCount,
    MsgSuccessCount,
    Weights,
    LastReconcileHeight,
    InflightFeePayable,
    InflightRewardsReceivable,
    InflightUnbond,
    InflightDelegation,
    InflightDeposit
];

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum Event {
    UndelegatedAssetsTransferred,
    DepositsTransferred(u128),
    UnbondComplete(u128),
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

        dbg!(rewards_balance_report);
        dbg!(last_reconcile_height);

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
}

pub struct FsmImpl<'a> {
    config: &'a dyn Config,
    repo: &'a dyn Repository,
    env: &'a dyn Env,
}

pub fn fsm<'a>(config: &'a dyn Config, repo: &'a dyn Repository, env: &'a dyn Env) -> FsmImpl<'a> {
    FsmImpl { config, repo, env }
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
    weights: Option<Weights>,
}

impl Cache {
    fn into_cmds(self) -> Vec<Cmd> {
        [
            self.clear_redelegation
                .then_some(Cmd::ClearRedelegationRequest),
            self.delegated.map(Cmd::from),
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
            self.weights.map(Cmd::from),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

struct IntermediateState<'a> {
    repo: &'a dyn Repository,
    cache: Cache,
}

impl<'a> IntermediateState<'a> {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::ClearRedelegationRequest => self.cache.clear_redelegation = true,
            Cmd::InflightDeposit(v) => self.cache.inflight_deposit = Some(v),
            Cmd::InflightDelegation(v) => self.cache.inflight_delegation = Some(v),
            Cmd::InflightUnbond(v) => self.cache.inflight_unbond = Some(v),
            Cmd::InflightRewardsReceivable(v) => self.cache.inflight_rewards_receivable = Some(v),
            Cmd::InflightFeePayable(v) => self.cache.inflight_fee_payable = Some(v),
            Cmd::LastReconcileHeight(v) => self.cache.last_reconcile_height = Some(v),
            Cmd::Weights(v) => self.cache.weights = Some(v),
            Cmd::Delegated(v) => self.cache.delegated = Some(v),
            Cmd::PendingDeposit(v) => self.cache.pending_deposit = Some(v),
            Cmd::PendingUnbond(v) => self.cache.pending_unbond = Some(v),
            Cmd::MsgSuccessCount(v) => self.cache.msg_success_count = Some(v),
            Cmd::MsgIssuedCount(v) => self.cache.msg_issued_count = Some(v),
            _ => panic!("unexpected cmd: {cmd:?}"),
        }
    }
}

impl<'a> Repository for IntermediateState<'a> {
    fn delegated(&self) -> Delegated {
        self.cache
            .delegated
            .unwrap_or_else(|| self.repo.delegated())
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

    fn weights(&self) -> Weights {
        self.cache
            .weights
            .clone()
            .unwrap_or_else(|| self.repo.weights())
    }
}

type Handler = fn(&dyn Config, &dyn Repository, &dyn Env) -> Transition;

fn start_setup_rewards_address(
    _config: &dyn Config,
    _repo: &dyn Repository,
    env: &dyn Env,
) -> Transition {
    let Some((delegation_account, rewards_account)) = env
        .delegation_account_address()
        .zip(env.rewards_account_address())
    else {
        return Transition::abort();
    };

    let tx_msgs = TxMsgs::one(TxMsg::SetRewardsWithdrawalAddress(
        delegation_account,
        rewards_account,
    ));

    Transition::tx(tx_msgs, vec![])
}

fn on_setup_rewards_address_success(
    _config: &dyn Config,
    _repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    Transition::next(vec![])
}

fn start_setup_authz(_config: &dyn Config, _repo: &dyn Repository, env: &dyn Env) -> Transition {
    let (delegation_account, rewards_account) = env
        .delegation_account_address()
        .zip(env.rewards_account_address())
        .expect("always: there must be delegation and rewards addresses to access this phase");

    let tx_msgs = TxMsgs::one(TxMsg::GrantAuthzSend(rewards_account, delegation_account));

    Transition::tx(tx_msgs, vec![])
}

fn on_setup_authz_success(
    _config: &dyn Config,
    _repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    Transition::next(vec![])
}

struct Slashing {
    /// The post-slashing weights
    adjusted_weights: Weights,
    /// The post-slashing delegated amount
    delegated: u128,
}

// determine whether a slashing has occured
fn check_for_slashing(
    delegated: Delegated,
    last_reconcile_height: LastReconcileHeight,
    delegations: DelegationsReport,
) -> Option<Slashing> {
    let LastReconcileHeight(last_reconcile_height) = last_reconcile_height;

    let Delegated(delegated) = delegated;

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

    Some(Slashing {
        adjusted_weights,
        delegated: delegations.total_delegated,
    })
}

fn start_reconcile(_config: &dyn Config, repo: &dyn Repository, env: &dyn Env) -> Transition {
    let Some(last_reconcile_height) = repo.last_reconcile_height() else {
        return Transition::next(vec![]);
    };

    let Some(delegations) = env.delegations_report() else {
        return Transition::next(vec![]);
    };

    let Some(slashing) = check_for_slashing(repo.delegated(), last_reconcile_height, delegations)
    else {
        return Transition::next(vec![]);
    };

    let cmds = set![slashing.adjusted_weights, Delegated(slashing.delegated)];

    Transition::next(cmds)
}

fn start_redelegate(_config: &dyn Config, repo: &dyn Repository, env: &dyn Env) -> Transition {
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

    let tx_msgs = TxMsgs::one(TxMsg::Redelegate(ValidatorSetSlot(slot), *delegated_amount));

    Transition::tx(tx_msgs, vec![])
}

fn on_redelegate_success(
    _config: &dyn Config,
    _repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    Transition::next(set![Cmd::ClearRedelegationRequest]).event(Event::RedelegationSuccessful)
}

// Do not retry on failure, clear request and move on
fn on_redelegate_failure(
    _config: &dyn Config,
    _repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    Transition::next(set![Cmd::ClearRedelegationRequest])
}

fn undelegate_phase_msgs(weights: &Weights, unbond_amount: u128) -> impl Iterator<Item = TxMsg> {
    weights
        .split_balance(unbond_amount)
        .into_iter()
        // get the indexes of the slots
        .enumerate()
        // skip slots where the split amount is zero
        .filter_map(|(idx, amount)| Some(ValidatorSetSlot(idx)).zip(NonZeroU128::new(amount)))
        // create undelegate msg
        .map(|(slot, amount)| TxMsg::Undelegate(slot, amount.get()))
}

fn start_undelegate(config: &dyn Config, repo: &dyn Repository, _env: &dyn Env) -> Transition {
    let PendingUnbond(pending_unbond) = repo.pending_unbond();

    if pending_unbond == 0 {
        return Transition::next(vec![]);
    }

    let undelegate_msgs = undelegate_phase_msgs(&repo.weights(), pending_unbond);

    let Some(tx_msgs) = TxMsgBatcher::new(config, repo).batch_msgs(undelegate_msgs) else {
        return Transition::next(vec![]);
    };

    let cmds = set![InflightUnbond(pending_unbond)];

    Transition::tx(tx_msgs, cmds)
}

fn on_undelegate_success(config: &dyn Config, repo: &dyn Repository, _env: &dyn Env) -> Transition {
    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();

    let undelegate_msgs = undelegate_phase_msgs(&repo.weights(), inflight_unbond);

    if let Some(tx_msgs) = TxMsgBatcher::new(config, repo).batch_msgs(undelegate_msgs) {
        return Transition::tx(tx_msgs, vec![]);
    };

    let PendingUnbond(pending_unbond) = repo.pending_unbond();

    let Delegated(delegated) = repo.delegated();

    let cmds = set![
        PendingUnbond(pending_unbond - inflight_unbond),
        InflightUnbond(0),
        Delegated(delegated - inflight_unbond)
    ];

    Transition::next(cmds).event(Event::UnbondComplete(inflight_unbond))
}

fn retry_undelegate(config: &dyn Config, repo: &dyn Repository, _env: &dyn Env) -> Transition {
    let InflightUnbond(inflight_unbond) = repo.inflight_unbond();

    let undelegate_msgs = undelegate_phase_msgs(&repo.weights(), inflight_unbond);

    let tx_msgs = TxMsgBatcher::new(config, repo)
        .batch_msgs(undelegate_msgs)
        .expect("always: messages to re-issue when retrying");

    Transition::tx(tx_msgs, vec![])
}

fn start_transfer_undelegated(
    _config: &dyn Config,
    repo: &dyn Repository,
    env: &dyn Env,
) -> Transition {
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

    let tx_msgs = TxMsgs::one(TxMsg::TransferInUndelegated(amount));

    Transition::tx(tx_msgs, vec![])
}

fn on_transfer_undelegated_success(
    _config: &dyn Config,
    _repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    Transition::next(vec![]).event(Event::UndelegatedAssetsTransferred)
}

fn start_transfer_pending_deposits(
    _config: &dyn Config,
    repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
    let PendingDeposit(pending_deposit) = repo.pending_deposit();

    if pending_deposit == 0 {
        return Transition::next(vec![]);
    }

    let tx_msgs = TxMsgs::one(TxMsg::TransferOutPendingDeposit(pending_deposit));

    let cmds = set![InflightDeposit(pending_deposit)];

    Transition::tx(tx_msgs, cmds)
}

fn on_transfer_pending_deposits_success(
    _config: &dyn Config,
    repo: &dyn Repository,
    _env: &dyn Env,
) -> Transition {
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

fn delegate_phase_msgs(
    weights: &Weights,
    balances: DelegatePhaseBalances,
    fee_recipient: Option<FeeRecipient>,
) -> impl Iterator<Item = TxMsg> {
    let InflightDelegation(inflight_delegation) = balances.delegation;

    let delegate_msgs = weights
        .split_balance(inflight_delegation)
        .into_iter()
        // get the indexes of the slots
        .enumerate()
        // skip slots where the split amount is zero
        .filter_map(|(idx, amount)| (amount != 0).then_some((ValidatorSetSlot(idx), amount)))
        // create undelegate msg
        .map(|(slot, amount)| TxMsg::Delegate(slot, amount));

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

fn try_withdraw_rewards(config: &dyn Config, repo: &dyn Repository) -> Transition {
    if repo.last_reconcile_height().is_none() {
        return Transition::next(vec![]);
    }

    let weights = repo.weights();

    let msgs = weights
        .iter()
        .enumerate()
        .map(|(idx, _)| ValidatorSetSlot(idx))
        .map(TxMsg::WithdrawRewards);

    let Some(tx_msgs) = TxMsgBatcher::new(config, repo).batch_msgs(msgs) else {
        return Transition::next(vec![]);
    };

    Transition::tx(tx_msgs, vec![])
}

fn start_delegate(config: &dyn Config, repo: &dyn Repository, env: &dyn Env) -> Transition {
    let Some(delegate_balances) = delegate_phase_balances(config, repo, env) else {
        return try_withdraw_rewards(config, repo);
    };

    let delegate_msgs =
        delegate_phase_msgs(&repo.weights(), delegate_balances, env.fee_recipient());

    let tx_msgs = TxMsgBatcher::new(config, repo)
        .batch_msgs(delegate_msgs)
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

fn on_delegate_success(config: &dyn Config, repo: &dyn Repository, env: &dyn Env) -> Transition {
    let Some(delegate_balances) = delegate_phase_balances(config, repo, env) else {
        return try_withdraw_rewards(config, repo);
    };

    let delegate_msgs =
        delegate_phase_msgs(&repo.weights(), delegate_balances, env.fee_recipient());

    if let Some(tx_msgs) = TxMsgBatcher::new(config, repo).batch_msgs(delegate_msgs) {
        return Transition::tx(tx_msgs, vec![]);
    }

    let Delegated(delegated) = repo.delegated();

    let InflightDelegation(inflight_delegation) = delegate_balances.delegation;

    let cmds = set![
        Delegated(delegated + inflight_delegation),
        InflightDelegation(0),
        InflightDeposit(0),
        InflightRewardsReceivable(0),
        InflightFeePayable(0)
    ];

    Transition::next(cmds).event(Event::DelegationsIncreased(inflight_delegation))
}

fn handler(phase: Phase, state: State) -> Handler {
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

impl<'a> Fsm for FsmImpl<'a> {
    fn reconcile(&self) -> Response {
        let mut phase = self.repo.phase();
        let mut state = self.repo.state();

        let mut all_events = vec![];

        let mut intermediate_state = IntermediateState {
            repo: self.repo,
            cache: Cache::default(),
        };

        let mut tx_skip_count = 0;

        // issued messages were successful, adjust counts accordingly
        if state.is_pending() {
            let MsgIssuedCount(msg_issued_count) = intermediate_state.msg_issued_count();
            let MsgSuccessCount(msg_success_count) = intermediate_state.msg_success_count();

            intermediate_state
                .handle_cmd(MsgSuccessCount(msg_success_count + msg_issued_count).into());
        }

        loop {
            let Transition { kind, cmds, events } =
                handler(phase, state)(self.config, &intermediate_state, self.env);

            all_events.extend_from_slice(&events);

            for cmd in cmds {
                intermediate_state.handle_cmd(cmd);
            }

            match kind {
                TransitionKind::Next => {
                    intermediate_state.handle_cmd(MsgIssuedCount(0).into());
                    intermediate_state.handle_cmd(MsgSuccessCount(0).into());

                    // Possible Txs skipped
                    if state.is_idle() {
                        let phase_tx_count = phase.tx_count(
                            self.config.validator_set_size(),
                            self.config.max_msg_count(),
                        );

                        tx_skip_count += phase_tx_count;
                    }

                    let Some(next_phase) = phase.next() else {
                        let mut cmds = intermediate_state.cache.into_cmds();

                        let CurrentHeight(current_height) = self.env.current_height();

                        cmds.push(LastReconcileHeight(current_height).into());
                        cmds.push(Phase::StartReconcile.into());
                        cmds.push(State::Idle.into());

                        return Response {
                            cmds,
                            events: all_events,
                            tx_msgs: None,
                            tx_skip_count,
                        };
                    };

                    phase = next_phase;
                    state = State::Idle;
                }

                TransitionKind::Tx(tx_msgs) => {
                    intermediate_state.handle_cmd(MsgIssuedCount(tx_msgs.msgs.len()).into());

                    let mut cmds = intermediate_state.cache.into_cmds();

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
                        self.config.validator_set_size(),
                        self.config.max_msg_count(),
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

    fn failed(&self) -> Response {
        if !self.repo.state().is_pending() {
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
}

#[cfg(test)]
mod test;
