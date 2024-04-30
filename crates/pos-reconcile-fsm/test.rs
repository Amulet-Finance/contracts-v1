use test_utils::prelude::*;

use super::*;

#[derive(Default, serde::Serialize)]
struct Context {
    phase: Option<Phase>,
    state: Option<State>,
    weights: Option<Weights>,
    msg_issued_count: Option<MsgIssuedCount>,
    msg_success_count: Option<MsgSuccessCount>,
    delegated: Option<Delegated>,
    pending_deposit: Option<PendingDeposit>,
    pending_unbond: Option<PendingUnbond>,
    inflight_deposit: Option<InflightDeposit>,
    inflight_unbond: Option<InflightUnbond>,
    inflight_delegation: Option<InflightDelegation>,
    inflight_rewards_receivable: Option<InflightRewardsReceivable>,
    inflight_fee_payable: Option<InflightFeePayable>,
    last_reconcile_height: Option<LastReconcileHeight>,
    current_height: u64,
    rewards_balance_report: Option<RemoteBalanceReport>,
    redelegation_request: Option<RedelegationSlot>,
}

macro_rules! progress_fsm {
    ($ctx:ident) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx).reconcile();

        for cmd in response.cmds {
            $ctx.handle_cmd(cmd);
        }
    }};

    ($ctx:ident, $expect:expr) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx).reconcile();

        check(&response, $expect);

        for cmd in response.cmds {
            $ctx.handle_cmd(cmd);
        }
    }};
}

macro_rules! failure {
    ($ctx:ident) => {{
        let response = fsm(&$ctx, &$ctx, &$ctx).failed();

        for cmd in response.cmds {
            $ctx.handle_cmd(cmd);
        }
    }};
}

impl Context {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Phase(v) => self.phase = Some(v),
            Cmd::State(v) => self.state = Some(v),
            Cmd::InflightDeposit(v) => self.inflight_deposit = Some(v),
            Cmd::InflightDelegation(v) => self.inflight_delegation = Some(v),
            Cmd::InflightUnbond(v) => self.inflight_unbond = Some(v),
            Cmd::InflightRewardsReceivable(v) => self.inflight_rewards_receivable = Some(v),
            Cmd::InflightFeePayable(v) => self.inflight_fee_payable = Some(v),
            Cmd::LastReconcileHeight(v) => self.last_reconcile_height = Some(v),
            Cmd::Weights(v) => self.weights = Some(v),
            Cmd::MsgIssuedCount(v) => self.msg_issued_count = Some(v),
            Cmd::MsgSuccessCount(v) => self.msg_success_count = Some(v),
            Cmd::Delegated(v) => self.delegated = Some(v),
            Cmd::PendingDeposit(v) => self.pending_deposit = Some(v),
            Cmd::PendingUnbond(v) => self.pending_unbond = Some(v),
            Cmd::ClearRedelegationRequest => self.redelegation_request = None,
        }
    }

    #[allow(dead_code)]
    fn increment_height(&mut self) {
        self.current_height += 1
    }

    fn with_pending_deposit(mut self, pending_deposit: u128) -> Self {
        self.pending_deposit = Some(PendingDeposit(pending_deposit));
        self
    }

    fn with_rewards_balance_report(mut self, height: u64, amount: u128) -> Self {
        self.rewards_balance_report = Some(RemoteBalanceReport {
            height,
            amount: RemoteBalance(amount),
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

    fn validator_set_size(&self) -> ValidatorSetSize {
        ValidatorSetSize(5)
    }
}

impl Repository for Context {
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

    fn weights(&self) -> Weights {
        self.weights.clone().unwrap_or_else(|| {
            Weights::new(&[
                Weight::checked_from_bps(2000u32).unwrap(),
                Weight::checked_from_bps(2000u32).unwrap(),
                Weight::checked_from_bps(2000u32).unwrap(),
                Weight::checked_from_bps(2000u32).unwrap(),
                Weight::checked_from_bps(2000u32).unwrap(),
            ])
            .unwrap()
        })
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
        None
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

    progress_fsm!(
        ctx,
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
                )"#]]
    );

    progress_fsm!(
        ctx,
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
            )"#]]
    );

    progress_fsm!(
        ctx,
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
            )"#]]
    );

    progress_fsm!(
        ctx,
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
            )"#]]
    );

    progress_fsm!(
        ctx,
        expect![[r#"
            (
              cmds: [
                Delegated((200)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(200),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]]
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

    progress_fsm!(
        ctx,
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
                  Delegate((0), 24),
                  Delegate((1), 19),
                  Delegate((2), 19),
                  Delegate((3), 19),
                  Delegate((4), 19),
                ],
              )),
              tx_skip_count: 4,
            )"#]]
    );

    progress_fsm!(
        ctx,
        expect![[r#"
            (
              cmds: [
                Delegated((300)),
                InflightDelegation((0)),
                InflightDeposit((0)),
                InflightFeePayable((0)),
                InflightRewardsReceivable((0)),
                MsgIssuedCount((0)),
                MsgSuccessCount((0)),
                LastReconcileHeight((0)),
                Phase(StartReconcile),
                State(Idle),
              ],
              events: [
                DelegationsIncreased(100),
              ],
              tx_msgs: None,
              tx_skip_count: 0,
            )"#]]
    );
}

#[test]
fn nothing_to_do() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        ..Default::default()
    };

    progress_fsm!(
        ctx,
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
            )"#]]
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

    progress_fsm!(
        ctx,
        expect![[r#"
            (
              cmds: [
                MsgIssuedCount((5)),
                MsgSuccessCount((0)),
                Phase(Delegate),
                State(Pending),
              ],
              events: [],
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
            )"#]]
    );

    progress_fsm!(
        ctx,
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
            )"#]]
    );
}

#[test]
fn pending_unbond_only() {
    let mut ctx = Context {
        phase: Some(Phase::StartReconcile),
        state: Some(State::Idle),
        last_reconcile_height: Some(LastReconcileHeight(0)),
        delegated: Some(Delegated(1_000_000)),
        pending_unbond: Some(PendingUnbond(500_000)),
        ..Default::default()
    };

    progress_fsm!(
        ctx,
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
                  Undelegate((0), 100004),
                  Undelegate((1), 99999),
                  Undelegate((2), 99999),
                  Undelegate((3), 99999),
                  Undelegate((4), 99999),
                ],
              )),
              tx_skip_count: 1,
            )"#]]
    );

    progress_fsm!(
        ctx,
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
                UnbondComplete(500000),
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
            )"#]]
    );

    progress_fsm!(
        ctx,
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
            )"#]]
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
