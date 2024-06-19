use std::collections::BTreeMap;

use test_utils::prelude::*;

use super::*;

fn weights(n_slots: u32) -> Weights {
    assert!(n_slots > 0);

    let weight = 10_000 / n_slots;
    let rem = 10_000 % weight;
    let mut weights = vec![weight; n_slots as usize];
    weights[0] += rem;

    let weights = weights
        .iter()
        .copied()
        .map(Weight::checked_from_bps)
        .collect::<Option<Vec<_>>>()
        .unwrap();

    Weights::new(&weights).unwrap()
}

#[derive(Default, serde::Serialize)]
struct Context {
    starting_weights: Option<Weights>,
    current_height: u64,
    delegate_start_slot: Option<DelegateStartSlot>,
    delegated: Option<Delegated>,
    delegations: BTreeMap<usize, u128>,
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
    phase: Option<Phase>,
    redelegation_request: Option<RedelegationSlot>,
    delegations_report: Option<DelegationsReport>,
    rewards_balance_report: Option<RemoteBalanceReport>,
    state: Option<State>,
    undelegate_start_slot: Option<UndelegateStartSlot>,
    weights: Option<Weights>,
}

macro_rules! progress_fsm {
    ($ctx:ident) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx).reconcile();

        for cmd in response.cmds.clone() {
            $ctx.handle_cmd(cmd);
        }

        if let Some(tx_msgs) = response.tx_msgs.clone() {
            for tx_msg in tx_msgs.msgs {
                $ctx.handle_tx_msg(tx_msg);
            }
        }

        response
    }};
}

macro_rules! force_next {
    ($ctx:ident) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx)
            .force_next()
            .expect("failed undelegate or delegate phase");

        for cmd in response.cmds.clone() {
            $ctx.handle_cmd(cmd);
        }

        if let Some(tx_msgs) = response.tx_msgs.clone() {
            for tx_msg in tx_msgs.msgs {
                $ctx.handle_tx_msg(tx_msg);
            }
        }

        response
    }};
}

macro_rules! failure {
    ($ctx:ident) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx).failed();

        for cmd in response.cmds {
            $ctx.handle_cmd(cmd);
        }

        assert!(response.tx_msgs.is_none());
    }};
}

impl Context {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::ClearRedelegationRequest => self.redelegation_request = None,
            Cmd::DelegateStartSlot(v) => self.delegate_start_slot = Some(v),
            Cmd::Delegated(v) => self.delegated = Some(v),
            Cmd::InflightDelegation(v) => self.inflight_delegation = Some(v),
            Cmd::InflightDeposit(v) => self.inflight_deposit = Some(v),
            Cmd::InflightFeePayable(v) => self.inflight_fee_payable = Some(v),
            Cmd::InflightRewardsReceivable(v) => self.inflight_rewards_receivable = Some(v),
            Cmd::InflightUnbond(v) => self.inflight_unbond = Some(v),
            Cmd::LastReconcileHeight(v) => self.last_reconcile_height = Some(v),
            Cmd::MsgIssuedCount(v) => self.msg_issued_count = Some(v),
            Cmd::MsgSuccessCount(v) => self.msg_success_count = Some(v),
            Cmd::PendingDeposit(v) => self.pending_deposit = Some(v),
            Cmd::PendingUnbond(v) => self.pending_unbond = Some(v),
            Cmd::Phase(v) => self.phase = Some(v),
            Cmd::State(v) => self.state = Some(v),
            Cmd::UndelegateStartSlot(v) => self.undelegate_start_slot = Some(v),
            Cmd::Weights(v) => self.weights = Some(v),
        }
    }

    fn handle_tx_msg(&mut self, tx_msg: TxMsg) {
        match tx_msg {
            TxMsg::Undelegate(ValidatorSetSlot(slot), amount) => {
                *self.delegations.entry(slot).or_default() -= amount;
            }
            TxMsg::Delegate(ValidatorSetSlot(slot), amount) => {
                *self.delegations.entry(slot).or_default() += amount;
            }
            _ => {}
        }
    }

    fn with_current_height(mut self, height: u64) -> Self {
        self.current_height = height;
        self
    }

    fn with_pending_deposit(mut self, pending_deposit: u128) -> Self {
        self.pending_deposit = Some(PendingDeposit(pending_deposit));
        self
    }

    fn with_pending_unbond(mut self, pending_unbond: u128) -> Self {
        self.pending_unbond = Some(PendingUnbond(pending_unbond));
        self
    }

    fn with_rewards_balance_report(mut self, height: u64, amount: u128) -> Self {
        self.rewards_balance_report = Some(RemoteBalanceReport {
            height,
            amount: RemoteBalance(amount),
        });
        self
    }

    fn with_delegations_report(
        mut self,
        height: u64,
        total_delegated: u128,
        delegated_amounts_per_slot: Vec<u128>,
    ) -> Self {
        assert_eq!(
            delegated_amounts_per_slot.len(),
            self.validator_set_size().0
        );

        self.delegations_report = Some(DelegationsReport {
            height,
            total_delegated,
            delegated_amounts_per_slot,
        });
        self
    }
}

impl Config for Context {
    fn unbonding_time(&self) -> UnbondingTimeSecs {
        UnbondingTimeSecs(600)
    }

    fn max_msg_count(&self) -> MaxMsgCount {
        MaxMsgCount(16)
    }

    fn fee_payout_cooldown(&self) -> FeePaymentCooldownBlocks {
        FeePaymentCooldownBlocks(100)
    }

    fn fee_bps_block_increment(&self) -> FeeBpsBlockIncrement {
        FeeBpsBlockIncrement(1)
    }

    fn max_fee_bps(&self) -> MaxFeeBps {
        MaxFeeBps(200)
    }

    fn starting_weights(&self) -> Weights {
        if let Some(weights) = self.starting_weights.as_ref() {
            return weights.clone();
        }

        let ValidatorSetSize(n) = self.validator_set_size();

        weights(n as u32)
    }

    fn validator_set_size(&self) -> ValidatorSetSize {
        let size = self
            .starting_weights
            .as_ref()
            .map_or(5, |ws| ws.as_slice().len());

        ValidatorSetSize(size)
    }
}

impl Repository for Context {
    fn delegate_start_slot(&self) -> DelegateStartSlot {
        self.delegate_start_slot.unwrap_or_default()
    }

    fn delegated(&self) -> Delegated {
        self.delegated.unwrap_or_default()
    }

    fn inflight_delegation(&self) -> InflightDelegation {
        self.inflight_delegation.unwrap_or_default()
    }

    fn inflight_deposit(&self) -> InflightDeposit {
        self.inflight_deposit.unwrap_or_default()
    }

    fn inflight_fee_payable(&self) -> InflightFeePayable {
        self.inflight_fee_payable.unwrap_or_default()
    }

    fn inflight_rewards_receivable(&self) -> InflightRewardsReceivable {
        self.inflight_rewards_receivable.unwrap_or_default()
    }

    fn inflight_unbond(&self) -> InflightUnbond {
        self.inflight_unbond.unwrap_or_default()
    }

    fn last_reconcile_height(&self) -> Option<LastReconcileHeight> {
        self.last_reconcile_height
    }

    fn msg_issued_count(&self) -> MsgIssuedCount {
        self.msg_issued_count.unwrap_or_default()
    }

    fn msg_success_count(&self) -> MsgSuccessCount {
        self.msg_success_count.unwrap_or_default()
    }

    fn pending_deposit(&self) -> PendingDeposit {
        self.pending_deposit.unwrap_or_default()
    }

    fn pending_unbond(&self) -> PendingUnbond {
        self.pending_unbond.unwrap_or_default()
    }

    fn phase(&self) -> Phase {
        self.phase.unwrap_or_default()
    }

    fn state(&self) -> State {
        self.state.unwrap_or_default()
    }

    fn redelegation_slot(&self) -> Option<RedelegationSlot> {
        self.redelegation_request.clone()
    }

    fn undelegate_start_slot(&self) -> UndelegateStartSlot {
        self.undelegate_start_slot.unwrap_or_default()
    }

    fn weights(&self) -> Weights {
        self.weights
            .clone()
            .unwrap_or_else(|| self.starting_weights())
    }
}

impl Env for Context {
    fn current_height(&self) -> CurrentHeight {
        CurrentHeight(self.current_height)
    }

    fn now(&self) -> Now {
        Now(self.current_height)
    }

    fn delegation_account_address(&self) -> Option<Account> {
        Some("delegation_account".to_owned())
    }

    fn rewards_account_address(&self) -> Option<Account> {
        Some("rewards_account".to_owned())
    }

    fn fee_recipient(&self) -> Option<FeeRecipient> {
        None
    }

    fn delegations_report(&self) -> Option<DelegationsReport> {
        self.delegations_report.clone()
    }

    fn rewards_balance_report(&self) -> Option<RemoteBalanceReport> {
        self.rewards_balance_report
    }

    fn undelegated_balance_report(&self) -> Option<UndelegatedBalanceReport> {
        None
    }
}

#[test]
fn initial_deposit() {
    let mut ctx = Context::default().with_pending_deposit(200);

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
                (
                  cmds: [
                    MsgIssuedCount((1)),
                    Phase(SetupRewardsAddress),
                    State(Pending),
                  ],
                  events: [],
                  tx_msgs: Some((
                    msgs: [
                      SetRewardsWithdrawalAddress("delegation_account", "rewards_account"),
                    ],
                  )),
                  tx_skip_count: 0,
                )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((1)),
                MsgSuccessCount((0)),
                Phase(SetupAuthz),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  GrantAuthzSend("rewards_account", "delegation_account"),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDeposit((200)),
                MsgIssuedCount((1)),
                MsgSuccessCount((0)),
                Phase(TransferPendingDeposits),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  TransferOutPendingDeposit(200),
                ],
              )),
              tx_skip_count: 3,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((200)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                PendingDeposit((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                DepositsTransferred(200),
              ],
              tx_msgs: Some((
                msgs: [
                  Delegate((0), 44),
                  Delegate((1), 39),
                  Delegate((2), 39),
                  Delegate((3), 39),
                  Delegate((4), 39),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((200)),
                DelegateStartSlot((0)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.21999999999999999999999999999999")),
                  (("0.19499999999999999999999999999999")),
                  (("0.19499999999999999999999999999999")),
                  (("0.19499999999999999999999999999999")),
                  (("0.19499999999999999999999999999999")),
                ])),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(200),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );
}

#[test]
fn collect_rewards() {
    let mut ctx = Context::default().with_pending_deposit(200);

    // Setup Phase 1
    progress_fsm!(ctx);
    // Setup Phase 2
    progress_fsm!(ctx);
    // Transfer Deposit Phase
    progress_fsm!(ctx);
    // Delegate Phase
    progress_fsm!(ctx);
    // Complete
    progress_fsm!(ctx);

    let mut ctx = ctx.with_rewards_balance_report(1, 100);

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((100)),
                InflightRewardsReceivable((100)),
                MsgIssuedCount((6)),
                MsgSuccessCount((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Authz([
                    SendRewardsReceivable((100)),
                  ]),
                  Delegate((0), 18),
                  Delegate((1), 22),
                  Delegate((2), 20),
                  Delegate((3), 20),
                  Delegate((4), 20),
                ],
              )),
              tx_skip_count: 4,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((300)),
                DelegateStartSlot((0)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.20333333333333333333333333333333")),
                  (("0.19999999999999999999999999999999")),
                  (("0.19333333333333333333333333333333")),
                  (("0.19333333333333333333333333333333")),
                  (("0.19333333333333333333333333333333")),
                ])),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(100),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );
}

#[test]
fn nothing_to_do() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        ..Default::default()
    };

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [],
              tx_msgs: None,
              tx_skip_count: 5,
            )"#]],
    );
}

#[test]
fn withdraw_rewards_only() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        last_reconcile_height: Some(LastReconcileHeight(0)),
        ..Default::default()
    };

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [],
              tx_msgs: None,
              tx_skip_count: 5,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [],
              tx_msgs: None,
              tx_skip_count: 5,
            )"#]],
    );
}

#[test]
fn pending_unbond_only() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        last_reconcile_height: Some(LastReconcileHeight(0)),
        ..Default::default()
    }
    .with_pending_deposit(1_000_000);

    // Transfer
    progress_fsm!(ctx);

    // Delegate
    progress_fsm!(ctx);

    // Complete
    progress_fsm!(ctx);

    let mut ctx = ctx.with_pending_unbond(500_000);

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightUnbond((500000)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                Phase(Undelegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Undelegate((0), 100001),
                  Undelegate((1), 99999),
                  Undelegate((2), 99999),
                  Undelegate((3), 99999),
                  Undelegate((4), 99999),
                ],
              )),
              tx_skip_count: 1,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((500000)),
                InflightUnbond((0)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                PendingUnbond((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                UnbondStarted(500000),
              ],
              tx_msgs: Some((
                msgs: [
                  WithdrawRewards((0)),
                  WithdrawRewards((1)),
                  WithdrawRewards((2)),
                  WithdrawRewards((3)),
                  WithdrawRewards((4)),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    check(
        ctx.delegations,
        expect![[r#"
            {
              0: 100003,
              1: 100000,
              2: 100000,
              3: 100000,
              4: 100000,
            }"#]],
    );
}

#[test]
#[should_panic]
fn notify_failure_in_idle_state() {
    let mut ctx = Context::default();

    failure!(ctx);
}

#[test]
fn notify_failure_in_pending_state() {
    let mut ctx = Context::default();

    progress_fsm!(ctx);

    let response = fsm(&ctx, &ctx, &ctx).failed();

    check(
        &response,
        expect![[r#"
        (
          cmds: [
            State(Failed),
            MsgIssuedCount((0)),
          ],
          events: [],
          tx_msgs: None,
          tx_skip_count: 0,
        )"#]],
    );

    for cmd in response.cmds {
        ctx.handle_cmd(cmd);
    }
}

#[test]
#[should_panic]
fn notify_failure_in_failed_state() {
    let mut ctx = Context::default();

    progress_fsm!(ctx);

    failure!(ctx);

    failure!(ctx);
}

#[test]
fn pending_deposits_unbonded_before_delegation() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        last_reconcile_height: Some(LastReconcileHeight(0)),
        pending_deposit: Some(PendingDeposit(500_000)),
        pending_unbond: Some(PendingUnbond(500_000)),
        ..Default::default()
    };

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
        (
          cmds: [
            InflightDeposit((500000)),
            MsgIssuedCount((1)),
            MsgSuccessCount((0)),
            Phase(TransferPendingDeposits),
            State(Pending),
          ],
          events: [],
          tx_msgs: Some((
            msgs: [
              TransferOutPendingDeposit(500000),
            ],
          )),
          tx_skip_count: 3,
        )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((500000)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                PendingDeposit((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                DepositsTransferred(500000),
              ],
              tx_msgs: Some((
                msgs: [
                  Delegate((0), 100004),
                  Delegate((1), 99999),
                  Delegate((2), 99999),
                  Delegate((3), 99999),
                  Delegate((4), 99999),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((500000)),
                DelegateStartSlot((0)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.20000799999999999999999999999999")),
                  (("0.19999799999999999999999999999999")),
                  (("0.19999799999999999999999999999999")),
                  (("0.19999799999999999999999999999999")),
                  (("0.19999799999999999999999999999999")),
                ])),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(500000),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightUnbond((500000)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                Phase(Undelegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Undelegate((0), 100003),
                  Undelegate((1), 99998),
                  Undelegate((2), 99998),
                  Undelegate((3), 99998),
                  Undelegate((4), 99998),
                ],
              )),
              tx_skip_count: 1,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((0)),
                InflightUnbond((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                PendingUnbond((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                UnbondStarted(500000),
              ],
              tx_msgs: None,
              tx_skip_count: 3,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [],
              tx_msgs: None,
              tx_skip_count: 5,
            )"#]],
    );

    check(
        ctx.delegations,
        expect![[r#"
            {
              0: 1,
              1: 1,
              2: 1,
              3: 1,
              4: 1,
            }"#]],
    );
}

#[test]
fn total_unbonding() {
    let deposits: Vec<Vec<u128>> = vec![
        vec![1_000_000, 200_000, 500_000, 123_456_789],
        vec![500_000, 600_000, 700_000, 800_000],
        vec![2_000, 3_000, 4_000, 5_000],
        vec![50, 100, 150, 200],
        vec![999_999, 888_888, 777_777, 666_666],
        vec![1, 2, 3, 4],
        vec![100_000, 200_000, 300_000, 400_000],
        vec![10, 20, 30, 40],
        vec![123_123, 234_234, 345_345, 456_456],
        vec![123, 456, 789, 1011],
        vec![1500, 2500, 3500, 4500],
        vec![10_000, 20_000, 30_000, 40_000],
        vec![5, 10, 15, 20],
    ];

    for deposit_seq in deposits {
        let mut ctx = Context::default();

        for deposit in deposit_seq {
            ctx = ctx.with_pending_deposit(deposit);

            while progress_fsm!(ctx).tx_msgs.is_some() {}
        }

        let Delegated(delegated) = ctx.delegated();

        assert!(delegated > 0);

        ctx = ctx.with_pending_unbond(delegated);

        while progress_fsm!(ctx).tx_msgs.is_some() {}

        assert_eq!(ctx.delegated, Some(Delegated(0)));
    }
}

#[test]
fn detect_slashing() {
    let mut ctx = Context::default();

    ctx = ctx.with_pending_deposit(1_234_567_891);

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    check(
        &ctx.delegations,
        expect![[r#"
        {
          0: 246913579,
          1: 246913578,
          2: 246913578,
          3: 246913578,
          4: 246913578,
        }"#]],
    );

    let slashed_delegations = vec![
        246913579, 246913578, 246913578, 246913578, 239506170, // slashed 3%
    ];

    let total_delegated = slashed_delegations.iter().sum();

    let report_height = ctx.last_reconcile_height.as_ref().unwrap().0 + 1;

    ctx = ctx
        .with_delegations_report(report_height, total_delegated, slashed_delegations)
        .with_current_height(report_height + 1);

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((1227160483)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.20120724421990697365048708140319")),
                  (("0.20120724340501763044467265492935")),
                  (("0.20120724340501763044467265492935")),
                  (("0.20120724340501763044467265492935")),
                  (("0.19517102556504013501549495380874")),
                ])),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                SlashDetected(("0.99399999947025999560845536359409")),
              ],
              tx_msgs: Some((
                msgs: [
                  WithdrawRewards((0)),
                  WithdrawRewards((1)),
                  WithdrawRewards((2)),
                  WithdrawRewards((3)),
                  WithdrawRewards((4)),
                ],
              )),
              tx_skip_count: 4,
            )"#]],
    );

    // finish round
    while progress_fsm!(ctx).tx_msgs.is_some() {}

    ctx = ctx.with_pending_unbond(total_delegated);

    // next round
    while progress_fsm!(ctx).tx_msgs.is_some() {}

    assert_eq!(ctx.delegated, Some(Delegated(0)));
}

#[test]
fn undelegate_force_next() {
    let mut ctx = Context {
        starting_weights: Some(weights(20)),
        ..Default::default()
    }
    .with_pending_deposit(1_000_000_000);

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    ctx = ctx.with_pending_unbond(500_000_000);

    check(
        &ctx.weights,
        expect![[r#"
        Some(([
          (("0.05000001899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
          (("0.04999999899999999999999999999999")),
        ]))"#]],
    );

    // first undelegate batch
    progress_fsm!(ctx);

    check(
        &ctx.delegations,
        expect![[r#"
            {
              0: 25000010,
              1: 25000000,
              2: 25000000,
              3: 25000000,
              4: 25000000,
              5: 25000000,
              6: 25000000,
              7: 25000000,
              8: 25000000,
              9: 25000000,
              10: 25000000,
              11: 25000000,
              12: 25000000,
              13: 25000000,
              14: 25000000,
              15: 25000000,
              16: 49999999,
              17: 49999999,
              18: 49999999,
              19: 49999999,
            }"#]],
    );

    assert_eq!(ctx.phase, Some(Phase::Undelegate));

    // second undelegate batch
    let response = fsm(&ctx, &ctx, &ctx).reconcile();

    for cmd in response.cmds.clone() {
        ctx.handle_cmd(cmd);
    }

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((4)),
                MsgSuccessCount((16)),
                Phase(Undelegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Undelegate((16), 24999999),
                  Undelegate((17), 24999999),
                  Undelegate((18), 24999999),
                  Undelegate((19), 24999999),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    assert_eq!(ctx.phase, Some(Phase::Undelegate));
    assert_eq!(ctx.state, Some(State::Pending));

    failure!(ctx);

    let response = force_next!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((600000006)),
                InflightUnbond((100000006)),
                MsgIssuedCount((16)),
                MsgSuccessCount((0)),
                PendingUnbond((100000006)),
                UndelegateStartSlot((16)),
                Weights(([
                  (("0.04166668124999985416666812499998")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.04166666458333335416666645833333")),
                  (("0.08333332916666670833333291666667")),
                  (("0.08333332916666670833333291666667")),
                  (("0.08333332916666670833333291666667")),
                  (("0.08333332916666670833333291666667")),
                ])),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                UnbondStarted(399999994),
              ],
              tx_msgs: Some((
                msgs: [
                  WithdrawRewards((0)),
                  WithdrawRewards((1)),
                  WithdrawRewards((2)),
                  WithdrawRewards((3)),
                  WithdrawRewards((4)),
                  WithdrawRewards((5)),
                  WithdrawRewards((6)),
                  WithdrawRewards((7)),
                  WithdrawRewards((8)),
                  WithdrawRewards((9)),
                  WithdrawRewards((10)),
                  WithdrawRewards((11)),
                  WithdrawRewards((12)),
                  WithdrawRewards((13)),
                  WithdrawRewards((14)),
                  WithdrawRewards((15)),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    // finish round
    while progress_fsm!(ctx).tx_msgs.is_some() {}

    // resume undelegation
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((4)),
                MsgSuccessCount((0)),
                Phase(Undelegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Undelegate((16), 25000001),
                  Undelegate((17), 25000001),
                  Undelegate((18), 25000001),
                  Undelegate((19), 25000001),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    // resume undelegation
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((500000000)),
                InflightUnbond((0)),
                MsgIssuedCount((16)),
                MsgSuccessCount((0)),
                PendingUnbond((0)),
                UndelegateStartSlot((0)),
                Weights(([
                  (("0.05000001599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999599999999999999999999999")),
                  (("0.04999999199999999999999999999999")),
                  (("0.04999999199999999999999999999999")),
                  (("0.04999999199999999999999999999999")),
                  (("0.04999999199999999999999999999999")),
                ])),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                UnbondStarted(100000006),
              ],
              tx_msgs: Some((
                msgs: [
                  WithdrawRewards((0)),
                  WithdrawRewards((1)),
                  WithdrawRewards((2)),
                  WithdrawRewards((3)),
                  WithdrawRewards((4)),
                  WithdrawRewards((5)),
                  WithdrawRewards((6)),
                  WithdrawRewards((7)),
                  WithdrawRewards((8)),
                  WithdrawRewards((9)),
                  WithdrawRewards((10)),
                  WithdrawRewards((11)),
                  WithdrawRewards((12)),
                  WithdrawRewards((13)),
                  WithdrawRewards((14)),
                  WithdrawRewards((15)),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    check(
        ctx.delegations,
        expect![[r#"
            {
              0: 25000010,
              1: 25000000,
              2: 25000000,
              3: 25000000,
              4: 25000000,
              5: 25000000,
              6: 25000000,
              7: 25000000,
              8: 25000000,
              9: 25000000,
              10: 25000000,
              11: 25000000,
              12: 25000000,
              13: 25000000,
              14: 25000000,
              15: 25000000,
              16: 24999998,
              17: 24999998,
              18: 24999998,
              19: 24999998,
            }"#]],
    )
}

#[test]
fn undelegate_all_force_next() {
    let mut ctx = Context {
        starting_weights: Some(weights(20)),
        ..Default::default()
    }
    .with_pending_deposit(1_000_000_000);

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    ctx = ctx.with_pending_unbond(1_000_000_000);

    // first undelegate batch
    progress_fsm!(ctx);

    assert_eq!(ctx.phase, Some(Phase::Undelegate));

    // second undelegate batch
    let response = fsm(&ctx, &ctx, &ctx).reconcile();

    for cmd in response.cmds.clone() {
        ctx.handle_cmd(cmd);
    }

    assert_eq!(ctx.phase, Some(Phase::Undelegate));
    assert_eq!(ctx.state, Some(State::Pending));

    failure!(ctx);

    let response = force_next!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((200000012)),
                InflightUnbond((200000012)),
                MsgIssuedCount((16)),
                MsgSuccessCount((0)),
                PendingUnbond((200000012)),
                UndelegateStartSlot((16)),
                Weights(([
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.0")),
                  (("0.24999997500000149999991000000539")),
                  (("0.24999997500000149999991000000539")),
                  (("0.24999997500000149999991000000539")),
                  (("0.24999997500000149999991000000539")),
                ])),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                UnbondStarted(799999988),
              ],
              tx_msgs: Some((
                msgs: [
                  WithdrawRewards((0)),
                  WithdrawRewards((1)),
                  WithdrawRewards((2)),
                  WithdrawRewards((3)),
                  WithdrawRewards((4)),
                  WithdrawRewards((5)),
                  WithdrawRewards((6)),
                  WithdrawRewards((7)),
                  WithdrawRewards((8)),
                  WithdrawRewards((9)),
                  WithdrawRewards((10)),
                  WithdrawRewards((11)),
                  WithdrawRewards((12)),
                  WithdrawRewards((13)),
                  WithdrawRewards((14)),
                  WithdrawRewards((15)),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    // resume undelegation
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((4)),
                MsgSuccessCount((0)),
                Phase(Undelegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Undelegate((16), 49999997),
                  Undelegate((17), 49999997),
                  Undelegate((18), 49999997),
                  Undelegate((19), 49999997),
                ],
              )),
              tx_skip_count: 2,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((0)),
                InflightUnbond((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                PendingUnbond((0)),
                UndelegateStartSlot((0)),
                Weights(([
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                  (("0.04999999999999999999999999999999")),
                ])),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                UnbondStarted(200000012),
              ],
              tx_msgs: None,
              tx_skip_count: 4,
            )"#]],
    );
}

#[test]
fn delegate_force_next() {
    let mut ctx = Context {
        starting_weights: Some(weights(20)),
        ..Default::default()
    }
    .with_pending_deposit(1_000_000_000);

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    ctx = ctx
        .with_rewards_balance_report(1, 1_000_000)
        .with_current_height(2)
        .with_pending_deposit(200_000_000);

    // transfer out deposits
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
        (
          cmds: [
            InflightDeposit((200000000)),
            MsgIssuedCount((1)),
            MsgSuccessCount((0)),
            Phase(TransferPendingDeposits),
            State(Pending),
          ],
          events: [],
          tx_msgs: Some((
            msgs: [
              TransferOutPendingDeposit(200000000),
            ],
          )),
          tx_skip_count: 5,
        )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((201000000)),
                InflightRewardsReceivable((1000000)),
                MsgIssuedCount((16)),
                MsgSuccessCount((0)),
                PendingDeposit((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [
                DepositsTransferred(200000000),
              ],
              tx_msgs: Some((
                msgs: [
                  Authz([
                    SendRewardsReceivable((1000000)),
                  ]),
                  Delegate((0), 10049996),
                  Delegate((1), 10050004),
                  Delegate((2), 10050000),
                  Delegate((3), 10050000),
                  Delegate((4), 10050000),
                  Delegate((5), 10050000),
                  Delegate((6), 10050000),
                  Delegate((7), 10050000),
                  Delegate((8), 10050000),
                  Delegate((9), 10050000),
                  Delegate((10), 10050000),
                  Delegate((11), 10050000),
                  Delegate((12), 10050000),
                  Delegate((13), 10050000),
                  Delegate((14), 10050000),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    let response = fsm(&ctx, &ctx, &ctx).reconcile();

    for cmd in response.cmds.clone() {
        ctx.handle_cmd(cmd);
    }

    failure!(ctx);

    let response = force_next!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((1150750000)),
                InflightDelegation((0)),
                InflightDeposit((50250000)),
                InflightRewardsReceivable((0)),
                InflightFeePayable((0)),
                DelegateStartSlot((15)),
                Weights(([
                  (("0.05218337084510102107321312187703")),
                  (("0.0521833604171192700412774277645")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.05218335694112535303063219639365")),
                  (("0.04344992222463610688681294807734")),
                  (("0.04344992222463610688681294807734")),
                  (("0.04344992222463610688681294807734")),
                  (("0.04344992222463610688681294807734")),
                  (("0.04344992222463610688681294807734")),
                ])),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((2)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(150750000),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((50250000)),
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Delegate((15), 10050004),
                  Delegate((16), 10049999),
                  Delegate((17), 10049999),
                  Delegate((18), 10049999),
                  Delegate((19), 10049999),
                ],
              )),
              tx_skip_count: 6,
            )"#]],
    );

    // resume delegation
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((1201000000)),
                DelegateStartSlot((0)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.05000001082431307243963363863447")),
                  (("0.05000000083263946711074104912572")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.04999999750208159866777685262281")),
                  (("0.05000000083263946711074104912572")),
                  (("0.04999999666944213155703580349708")),
                  (("0.04999999666944213155703580349708")),
                  (("0.04999999666944213155703580349708")),
                  (("0.04999999666944213155703580349708")),
                ])),
                LastReconcileHeight((2)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(50250000),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    check(
        ctx.delegations,
        expect![[r#"
            {
              0: 60050015,
              1: 60050003,
              2: 60049999,
              3: 60049999,
              4: 60049999,
              5: 60049999,
              6: 60049999,
              7: 60049999,
              8: 60049999,
              9: 60049999,
              10: 60049999,
              11: 60049999,
              12: 60049999,
              13: 60049999,
              14: 60049999,
              15: 60050003,
              16: 60049998,
              17: 60049998,
              18: 60049998,
              19: 60049998,
            }"#]],
    );
}

#[test]
fn delegate_force_next_rewards_only() {
    let mut ctx = Context {
        starting_weights: Some(weights(20)),
        ..Default::default()
    }
    .with_pending_deposit(1_000_000_000);

    while progress_fsm!(ctx).tx_msgs.is_some() {}

    ctx = ctx
        .with_rewards_balance_report(1, 1_000_000)
        .with_current_height(2);

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((1000000)),
                InflightRewardsReceivable((1000000)),
                MsgIssuedCount((16)),
                MsgSuccessCount((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Authz([
                    SendRewardsReceivable((1000000)),
                  ]),
                  Delegate((0), 49999),
                  Delegate((1), 50001),
                  Delegate((2), 50000),
                  Delegate((3), 50000),
                  Delegate((4), 50000),
                  Delegate((5), 50000),
                  Delegate((6), 50000),
                  Delegate((7), 50000),
                  Delegate((8), 50000),
                  Delegate((9), 50000),
                  Delegate((10), 50000),
                  Delegate((11), 50000),
                  Delegate((12), 50000),
                  Delegate((13), 50000),
                  Delegate((14), 50000),
                ],
              )),
              tx_skip_count: 6,
            )"#]],
    );

    let response = fsm(&ctx, &ctx, &ctx).reconcile();

    for cmd in response.cmds.clone() {
        ctx.handle_cmd(cmd);
    }

    check(
        response,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((5)),
                MsgSuccessCount((16)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Delegate((15), 50000),
                  Delegate((16), 50000),
                  Delegate((17), 50000),
                  Delegate((18), 50000),
                  Delegate((19), 50000),
                ],
              )),
              tx_skip_count: 0,
            )"#]],
    );

    failure!(ctx);

    let response = force_next!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((1000750000)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightRewardsReceivable((250000)),
                InflightFeePayable((0)),
                DelegateStartSlot((15)),
                Weights(([
                  (("0.05001250761928553584811391456407")),
                  (("0.05001248963277541843617287034723")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.05001248863352485635773170122408")),
                  (("0.04996252610542093429927554334249")),
                  (("0.04996252610542093429927554334249")),
                  (("0.04996252610542093429927554334249")),
                  (("0.04996252610542093429927554334249")),
                  (("0.04996252610542093429927554334249")),
                ])),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((2)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(750000),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                InflightDelegation((250000)),
                InflightRewardsReceivable((250000)),
                MsgIssuedCount((6)),
                MsgSuccessCount((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
              tx_msgs: Some((
                msgs: [
                  Authz([
                    SendRewardsReceivable((250000)),
                  ]),
                  Delegate((15), 50004),
                  Delegate((16), 49999),
                  Delegate((17), 49999),
                  Delegate((18), 49999),
                  Delegate((19), 49999),
                ],
              )),
              tx_skip_count: 6,
            )"#]],
    );

    // resume delegation
    let response = progress_fsm!(ctx);

    check(
        response,
        expect![[r#"
            (
              cmds: [
                Delegated((1001000000)),
                DelegateStartSlot((0)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                Weights(([
                  (("0.05000001598401598401598401598401")),
                  (("0.04999999800199800199800199800199")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.04999999700299700299700299700299")),
                  (("0.050000000999000999000999000999")),
                  (("0.04999999600399600399600399600399")),
                  (("0.04999999600399600399600399600399")),
                  (("0.04999999600399600399600399600399")),
                  (("0.04999999600399600399600399600399")),
                ])),
                LastReconcileHeight((2)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(250000),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]],
    );

    check(
        ctx.delegations,
        expect![[r#"
            {
              0: 50050018,
              1: 50050000,
              2: 50049999,
              3: 50049999,
              4: 50049999,
              5: 50049999,
              6: 50049999,
              7: 50049999,
              8: 50049999,
              9: 50049999,
              10: 50049999,
              11: 50049999,
              12: 50049999,
              13: 50049999,
              14: 50049999,
              15: 50050003,
              16: 50049998,
              17: 50049998,
              18: 50049998,
              19: 50049998,
            }"#]],
    );
}
