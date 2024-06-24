use anyhow::{bail, Result};
use cosmos_sdk_proto::cosmos::{
    authz::v1beta1::{GenericAuthorization, Grant, MsgExec, MsgGrant},
    bank::v1beta1::MsgSend,
    distribution::v1beta1::{MsgSetWithdrawAddress, MsgWithdrawDelegatorReward},
    staking::v1beta1::{MsgBeginRedelegate, MsgDelegate, MsgUndelegate},
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, coins, to_json_string, BankMsg, Coin, Deps, DepsMut, Env as CwEnv, MessageInfo, Response,
    Storage, SubMsg,
};
use cw_utils::must_pay;
use neutron_sdk::{
    bindings::{
        msg::{IbcFee, NeutronMsg},
        query::NeutronQuery,
        types::ProtobufAny,
    },
    interchain_queries::v047::queries as icq,
    query::min_ibc_fee::query_min_ibc_fee,
    sudo::msg::RequestPacketTimeoutHeight,
};
use prost::{Message, Name};

use amulet_ntrn::{IbcFeeExt, IBC_FEE_DENOM};
use pos_reconcile_fsm::{
    fsm,
    types::{
        Account, BalancesIcqResult, CurrentHeight, DelegateStartSlot, Delegated, Delegation,
        DelegationsIcqResult, DelegationsReport, FeeBpsBlockIncrement, FeePaymentCooldownBlocks,
        FeeRecipient, InflightDelegation, InflightDeposit, InflightFeePayable,
        InflightRewardsReceivable, InflightUnbond, LastReconcileHeight, MaxFeeBps, MaxMsgCount,
        MsgIssuedCount, MsgSuccessCount, Now as ReconcilePosNow, PendingDeposit, PendingUnbond,
        Phase, ReconcilerFee, RedelegationSlot, RemoteBalance, RemoteBalanceReport,
        RewardsReceivable, State, UnbondingTimeSecs, UndelegateStartSlot, UndelegatedBalanceReport,
        Validator, ValidatorSetSize, ValidatorSetSlot, Weights,
    },
    AuthzMsg, Cmd as ReconcileCmd, Config, Env as FsmEnv, Event, Fsm, Repository,
    Response as FsmResponse, TxMsg,
};

use crate::{msg::StrategyExecuteMsg, state::StorageExt, strategy, types::Ica};

pub enum Status {
    Success,
    Failure,
}

pub enum Source {
    Trigger(MessageInfo, Option<FeeRecipient>),
    Continuation(Status),
}

struct StorageWrapper<'a> {
    storage: &'a dyn Storage,
}

struct Env<'a> {
    deps: Deps<'a, NeutronQuery>,
    env: &'a CwEnv,
    fee_recipient: Option<FeeRecipient>,
}

impl<'a> Config for StorageWrapper<'a> {
    fn unbonding_time(&self) -> UnbondingTimeSecs {
        UnbondingTimeSecs(self.storage.unbonding_period())
    }

    fn max_msg_count(&self) -> MaxMsgCount {
        MaxMsgCount(self.storage.max_ibc_msg_count())
    }

    fn fee_payout_cooldown(&self) -> FeePaymentCooldownBlocks {
        FeePaymentCooldownBlocks(self.storage.fee_payment_cooldown_blocks())
    }

    fn fee_bps_block_increment(&self) -> FeeBpsBlockIncrement {
        FeeBpsBlockIncrement(self.storage.fee_bps_block_increment())
    }

    fn max_fee_bps(&self) -> MaxFeeBps {
        MaxFeeBps(self.storage.max_fee_bps() as _)
    }

    fn starting_weights(&self) -> Weights {
        Weights::new_unchecked(self.storage.validator_initial_weights())
    }

    fn validator_set_size(&self) -> ValidatorSetSize {
        ValidatorSetSize(self.storage.validator_set_size())
    }
}

impl<'a> Repository for StorageWrapper<'a> {
    fn delegated(&self) -> Delegated {
        self.storage.delegated()
    }

    fn delegate_start_slot(&self) -> DelegateStartSlot {
        self.storage.delegate_start_slot()
    }

    fn inflight_delegation(&self) -> InflightDelegation {
        self.storage.inflight_delegation()
    }

    fn inflight_deposit(&self) -> InflightDeposit {
        self.storage.inflight_deposit()
    }

    fn inflight_fee_payable(&self) -> InflightFeePayable {
        self.storage.inflight_fee_payable()
    }

    fn inflight_rewards_receivable(&self) -> InflightRewardsReceivable {
        self.storage.inflight_rewards_receivable()
    }

    fn inflight_unbond(&self) -> InflightUnbond {
        self.storage.inflight_unbond()
    }

    fn last_reconcile_height(&self) -> Option<LastReconcileHeight> {
        self.storage.last_reconcile_height()
    }

    fn msg_issued_count(&self) -> MsgIssuedCount {
        self.storage.msg_issued_count()
    }

    fn msg_success_count(&self) -> MsgSuccessCount {
        self.storage.msg_success_count()
    }

    fn pending_deposit(&self) -> PendingDeposit {
        self.storage.pending_deposit()
    }

    fn pending_unbond(&self) -> PendingUnbond {
        self.storage.pending_unbond()
    }

    fn phase(&self) -> Phase {
        self.storage.reconcile_phase()
    }

    fn state(&self) -> State {
        self.storage.reconcile_state()
    }

    fn redelegation_slot(&self) -> Option<RedelegationSlot> {
        self.storage
            .redelegate_slot()
            .map(ValidatorSetSlot)
            .map(RedelegationSlot)
    }

    fn redelegate_to_validator(&self) -> Option<Validator> {
        self.storage.redelegate_to()
    }

    fn undelegate_start_slot(&self) -> UndelegateStartSlot {
        self.storage.undelegate_start_slot()
    }

    fn weights(&self) -> Weights {
        Weights::new_unchecked(self.storage.validator_weights())
    }
}

fn query_balance(deps: Deps<NeutronQuery>, env: CwEnv, icq_id: u64) -> RemoteBalanceReport {
    let res = match icq::query_balance(deps, env, icq_id) {
        Ok(res) => res,
        Err(err) => {
            panic!("balance query for icq {icq_id} failed: {err}");
        }
    };

    let stake_denom = deps.storage.remote_denom();

    let coins = res
        .balances
        .coins
        .into_iter()
        .map(|coin| (coin.denom, RemoteBalance(coin.amount.u128())))
        .collect();

    BalancesIcqResult {
        last_submitted_height: res.last_submitted_local_height,
        coins,
    }
    .into_remote_balance_report(&stake_denom)
}

impl<'a> FsmEnv for Env<'a> {
    fn current_height(&self) -> CurrentHeight {
        CurrentHeight(self.env.block.height)
    }

    fn now(&self) -> ReconcilePosNow {
        ReconcilePosNow(self.env.block.time.seconds())
    }

    fn delegation_account_address(&self) -> Option<Account> {
        self.deps.storage.main_ica_address()
    }

    fn rewards_account_address(&self) -> Option<Account> {
        self.deps.storage.rewards_ica_address()
    }

    fn fee_recipient(&self) -> Option<FeeRecipient> {
        self.fee_recipient.clone()
    }

    fn delegations_report(&self) -> Option<DelegationsReport> {
        let icq_id = self.deps.storage.delegations_icq()?;

        let res = match icq::query_delegations(self.deps, self.env.clone(), icq_id) {
            Ok(res) => res,
            Err(err) => {
                panic!("delegations query for icq {icq_id} failed: {err}");
            }
        };

        let delegations = res
            .delegations
            .into_iter()
            .map(|d| Delegation {
                validator: d.validator,
                amount: d.amount.amount.u128(),
            })
            .collect();

        let validators = self.deps.storage.validators();

        DelegationsIcqResult {
            last_submitted_height: res.last_submitted_local_height,
            delegations,
        }
        .into_report(&validators)
    }

    fn rewards_balance_report(&self) -> Option<RemoteBalanceReport> {
        let icq_id = self.deps.storage.rewards_ica_balance_icq()?;

        let report = query_balance(self.deps, self.env.clone(), icq_id);

        Some(report)
    }

    fn undelegated_balance_report(&self) -> Option<UndelegatedBalanceReport> {
        let icq_id = self.deps.storage.main_ica_balance_icq()?;

        let last_updated_timestamp = self.deps.storage.last_main_ica_balance_icq_update()?;

        let remote_balance = query_balance(self.deps, self.env.clone(), icq_id);

        Some(UndelegatedBalanceReport {
            last_updated_timestamp,
            remote_balance,
        })
    }
}

#[derive(Default)]
struct ResponseBuilder {
    main_ica_msgs: Vec<ProtobufAny>,
    rewards_ica_msgs: Vec<ProtobufAny>,
    local_msgs: Vec<SubMsg<NeutronMsg>>,
}

impl ResponseBuilder {
    fn push_main_ica_msg(&mut self, msg: ProtobufAny) -> &mut Self {
        self.main_ica_msgs.push(msg);
        self
    }

    fn push_rewards_ica_msg(&mut self, msg: ProtobufAny) -> &mut Self {
        self.rewards_ica_msgs.push(msg);
        self
    }

    fn push_local_msg(&mut self, msg: SubMsg<NeutronMsg>) -> &mut Self {
        self.local_msgs.push(msg);
        self
    }

    fn build(self, storage: &dyn Storage, fee: IbcFee) -> Response<NeutronMsg> {
        let mut response = Response::default();

        if !self.local_msgs.is_empty() {
            response = response.add_submessages(self.local_msgs);
        }

        if self.main_ica_msgs.is_empty() && self.rewards_ica_msgs.is_empty() {
            return response;
        }

        let connection_id = storage.connection_id();

        let timeout = storage.interchain_tx_timeout_seconds();

        if !self.main_ica_msgs.is_empty() {
            let interchain_account_id = Ica::Main.id().to_owned();

            let interchain_tx = NeutronMsg::SubmitTx {
                connection_id: connection_id.clone(),
                interchain_account_id,
                msgs: self.main_ica_msgs,
                memo: String::new(),
                timeout,
                fee: fee.clone(),
            };

            response = response.add_message(interchain_tx);
        }

        if !self.rewards_ica_msgs.is_empty() {
            let interchain_account_id = Ica::Rewards.id().to_owned();

            let interchain_tx = NeutronMsg::SubmitTx {
                connection_id,
                interchain_account_id,
                msgs: self.rewards_ica_msgs,
                memo: String::new(),
                timeout,
                fee,
            };

            response = response.add_message(interchain_tx);
        }

        response
    }
}

fn handle_reconcile_cmd(storage: &mut dyn Storage, cmd: ReconcileCmd) {
    match cmd {
        ReconcileCmd::ClearRedelegationRequest => {
            storage.clear_redelegate_slot();
            storage.clear_redelegate_to();
        }
        ReconcileCmd::DelegateStartSlot(v) => storage.set_delegate_start_slot(v),
        ReconcileCmd::Delegated(v) => storage.set_delegated(v),
        ReconcileCmd::InflightDelegation(v) => storage.set_inflight_delegation(v),
        ReconcileCmd::InflightDeposit(v) => storage.set_inflight_deposit(v),
        ReconcileCmd::InflightFeePayable(v) => storage.set_inflight_fee_payable(v),
        ReconcileCmd::InflightRewardsReceivable(v) => storage.set_inflight_rewards_receivable(v),
        ReconcileCmd::InflightUnbond(v) => storage.set_inflight_unbond(v),
        ReconcileCmd::LastReconcileHeight(v) => storage.set_last_reconcile_height(v),
        ReconcileCmd::MsgIssuedCount(v) => storage.set_msg_issued_count(v),
        ReconcileCmd::MsgSuccessCount(v) => storage.set_msg_success_count(v),
        ReconcileCmd::PendingDeposit(v) => storage.set_pending_deposit(v),
        ReconcileCmd::PendingUnbond(v) => storage.set_pending_unbond(v),
        ReconcileCmd::Phase(v) => storage.set_reconcile_phase(v),
        ReconcileCmd::State(v) => storage.set_reconcile_state(v),
        ReconcileCmd::UndelegateStartSlot(v) => storage.set_undelegate_start_slot(v),
        ReconcileCmd::Weights(v) => storage.set_validator_weights(v),
    }
}

fn set_withdraw_address_msg(delegator_address: String, withdraw_address: String) -> ProtobufAny {
    let delegate_msg = MsgSetWithdrawAddress {
        delegator_address,
        withdraw_address,
    };

    let encoded = delegate_msg.encode_to_vec();

    ProtobufAny {
        type_url: MsgSetWithdrawAddress::type_url(),
        value: encoded.into(),
    }
}

fn grant_authz_send(granter: String, grantee: String) -> ProtobufAny {
    let auth = GenericAuthorization {
        msg: "/cosmos.bank.v1beta1.MsgSend".to_owned(),
    };

    let grant_msg = MsgGrant {
        granter,
        grantee,
        grant: Some(Grant {
            authorization: Some(cosmos_sdk_proto::Any {
                type_url: "/cosmos.authz.v1beta1.GenericAuthorization".to_owned(),
                value: auth.encode_to_vec(),
            }),
            expiration: Some(prost_types::Timestamp {
                seconds: 253_402_214_400,
                nanos: 0,
            }),
        }),
    };

    let encoded = grant_msg.encode_to_vec();

    ProtobufAny {
        type_url: "/cosmos.authz.v1beta1.MsgGrant".to_owned(),
        value: encoded.into(),
    }
}

fn transfer_in_undelegated(storage: &dyn Storage, env: &CwEnv, amount: u128) -> ProtobufAny {
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
    use cosmos_sdk_proto::ibc::core::client::v1::Height;

    #[derive(Clone, PartialEq, Message)]
    struct MsgTransfer {
        #[prost(string, tag = "1")]
        source_port: String,
        #[prost(string, tag = "2")]
        source_channel: String,
        #[prost(message, optional, tag = "3")]
        token: Option<Coin>,
        #[prost(string, tag = "4")]
        sender: String,
        #[prost(string, tag = "5")]
        receiver: String,
        #[prost(message, optional, tag = "6")]
        timeout_height: Option<Height>,
        #[prost(uint64, tag = "7")]
        timeout_timestamp: u64,
        #[prost(string, tag = "8")]
        memo: String,
    }

    #[cw_serde]
    struct IbcHookWasm<Msg> {
        contract: String,
        msg: Msg,
    }

    #[cw_serde]
    struct IbcHookMemo<Msg> {
        wasm: IbcHookWasm<Msg>,
    }

    let sender = storage
        .main_ica_address()
        .expect("must have main ica address for there to be undelegations");

    let source_channel = storage.transfer_in_channel();

    let contract = env.contract.address.clone();

    let timeout_seconds = storage.transfer_in_timeout_seconds();

    let timeout_timestamp_nanos = env.block.time.plus_seconds(timeout_seconds).nanos();

    let remote_denom = storage.remote_denom();

    let callback = StrategyExecuteMsg::ReceiveUndelegated {};

    let ibc_hook = IbcHookMemo {
        wasm: IbcHookWasm {
            contract: contract.clone().into_string(),
            msg: callback,
        },
    };

    let memo = to_json_string(&ibc_hook).expect("infallible serialization");

    let transfer_msg = MsgTransfer {
        source_port: "transfer".to_owned(),
        source_channel,
        token: Some(Coin {
            denom: remote_denom,
            amount: amount.to_string(),
        }),
        sender,
        receiver: contract.into_string(),
        timeout_height: None,
        timeout_timestamp: timeout_timestamp_nanos,
        memo,
    };

    ProtobufAny {
        type_url: "/ibc.applications.transfer.v1.MsgTransfer".to_owned(),
        value: transfer_msg.encode_to_vec().into(),
    }
}

fn transfer_out_pending_deposits(
    storage: &dyn Storage,
    env: &CwEnv,
    fee: &IbcFee,
    amount: u128,
) -> SubMsg<NeutronMsg> {
    let transfer_out_channel = storage.transfer_out_channel();

    let sender = env.contract.address.clone().into_string();

    let receiver = storage
        .main_ica_address()
        .expect("must have main ica address for a deposit transfer request to be issued");

    let ibc_denom = storage.ibc_deposit_asset();

    let token = cosmwasm_std::coin(amount, ibc_denom.as_str());

    let timeout_height = RequestPacketTimeoutHeight {
        revision_number: None,
        revision_height: None,
    };

    let timeout_seconds = storage.transfer_out_timeout_seconds();

    let timeout_timestamp = env.block.time.plus_seconds(timeout_seconds).nanos();

    let msg = NeutronMsg::IbcTransfer {
        source_port: "transfer".to_owned(),
        source_channel: transfer_out_channel,
        sender,
        receiver,
        token,
        timeout_height,
        timeout_timestamp,
        memo: String::new(),
        fee: fee.to_owned(),
    };

    SubMsg::new(msg)
}

fn withdraw_rewards(
    storage: &dyn Storage,
    ValidatorSetSlot(slot): ValidatorSetSlot,
) -> ProtobufAny {
    let delegator_address = storage
        .main_ica_address()
        .expect("must have main ica address for a rewards withdrawal request to be issued");

    let validator_address = storage.validator(slot);

    let msg = MsgWithdrawDelegatorReward {
        delegator_address,
        validator_address,
    };

    let encoded = msg.encode_to_vec();

    ProtobufAny {
        type_url: MsgWithdrawDelegatorReward::type_url(),
        value: encoded.into(),
    }
}

fn redelegate(
    storage: &dyn Storage,
    ValidatorSetSlot(slot): ValidatorSetSlot,
    validator_dst_address: Validator,
    amount: u128,
) -> ProtobufAny {
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

    let delegator_address = storage
        .main_ica_address()
        .expect("must have a main ica address address for request to be issued");

    let validator_src_address = storage.validator(slot);

    let remote_denom = storage.remote_denom();

    let msg = MsgBeginRedelegate {
        delegator_address,
        validator_src_address,
        validator_dst_address,
        amount: Some(Coin {
            denom: remote_denom,
            amount: amount.to_string(),
        }),
    };

    let encoded = msg.encode_to_vec();

    ProtobufAny {
        type_url: MsgBeginRedelegate::type_url(),
        value: encoded.into(),
    }
}

fn undelegate(
    storage: &dyn Storage,
    ValidatorSetSlot(slot): ValidatorSetSlot,
    amount: u128,
) -> ProtobufAny {
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

    let delegator_address = storage
        .main_ica_address()
        .expect("must have main ica address for an undelegate request to be issued");

    let validator_address = storage.validator(slot);

    let remote_denom = storage.remote_denom();

    let msg = MsgUndelegate {
        delegator_address,
        validator_address,
        amount: Some(Coin {
            denom: remote_denom,
            amount: amount.to_string(),
        }),
    };

    let encoded = msg.encode_to_vec();

    ProtobufAny {
        type_url: MsgUndelegate::type_url(),
        value: encoded.into(),
    }
}

fn delegate(
    storage: &dyn Storage,
    ValidatorSetSlot(slot): ValidatorSetSlot,
    amount: u128,
) -> ProtobufAny {
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

    let delegator_address = storage
        .main_ica_address()
        .expect("must have main ica address for an undelegate request to be issued");

    let validator_address = storage.validator(slot);

    let remote_denom = storage.remote_denom();

    let msg = MsgDelegate {
        delegator_address,
        validator_address,
        amount: Some(Coin {
            denom: remote_denom,
            amount: amount.to_string(),
        }),
    };

    let encoded = msg.encode_to_vec();

    ProtobufAny {
        type_url: MsgDelegate::type_url(),
        value: encoded.into(),
    }
}

fn rewards_ica_send(storage: &dyn Storage, to_address: String, amount: u128) -> prost_types::Any {
    use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

    let from_address = storage
        .rewards_ica_address()
        .expect("must have rewards ica address for an authz to be issued");

    let remote_denom = storage.remote_denom();

    let msg = MsgSend {
        from_address,
        to_address,
        amount: vec![Coin {
            denom: remote_denom,
            amount: amount.to_string(),
        }],
    };

    prost_types::Any {
        type_url: MsgSend::type_url(),
        value: msg.encode_to_vec(),
    }
}

fn send_rewards_receivable(
    storage: &dyn Storage,
    RewardsReceivable(amount): RewardsReceivable,
) -> prost_types::Any {
    let to_address = storage
        .main_ica_address()
        .expect("must have main ica address for an authz to be issued");

    rewards_ica_send(storage, to_address, amount)
}

fn send_reconciler_fee(
    storage: &dyn Storage,
    fee_recipient: FeeRecipient,
    ReconcilerFee(amount): ReconcilerFee,
) -> prost_types::Any {
    rewards_ica_send(storage, fee_recipient, amount)
}

fn authz(storage: &dyn Storage, msgs: Vec<AuthzMsg>) -> ProtobufAny {
    let mut authz_msgs = vec![];

    for msg in msgs {
        let protobuf_any = match msg {
            AuthzMsg::SendRewardsReceivable(rewards_receivable) => {
                send_rewards_receivable(storage, rewards_receivable)
            }
            AuthzMsg::SendFee(recipient, fee) => send_reconciler_fee(storage, recipient, fee),
        };

        authz_msgs.push(protobuf_any);
    }

    let grantee = storage
        .main_ica_address()
        .expect("must have main ica address for an authz to be issued");

    let msg = MsgExec {
        grantee,
        msgs: authz_msgs,
    };

    let encoded = msg.encode_to_vec();

    ProtobufAny {
        type_url: "/cosmos.authz.v1beta1.MsgExec".to_owned(),
        value: encoded.into(),
    }
}

fn handle_reconcile_tx_msg(
    storage: &mut dyn Storage,
    env: &CwEnv,
    response: &mut ResponseBuilder,
    fee: &IbcFee,
    tx_msg: TxMsg,
) {
    match tx_msg {
        TxMsg::SetRewardsWithdrawalAddress(delegator_addr, rewards_addr) => {
            let msg = set_withdraw_address_msg(delegator_addr, rewards_addr);

            response.push_main_ica_msg(msg);
        }

        TxMsg::GrantAuthzSend(granter, grantee) => {
            let msg = grant_authz_send(granter, grantee);

            response.push_rewards_ica_msg(msg);
        }

        TxMsg::TransferInUndelegated(amount) => {
            let msg = transfer_in_undelegated(storage, env, amount);

            response.push_main_ica_msg(msg);
        }

        TxMsg::TransferOutPendingDeposit(amount) => {
            let msg = transfer_out_pending_deposits(storage, env, fee, amount);

            response.push_local_msg(msg);
        }

        TxMsg::WithdrawRewards(slot) => {
            let msg = withdraw_rewards(storage, slot);

            response.push_main_ica_msg(msg);
        }

        TxMsg::Redelegate { slot, to, amount } => {
            let msg = redelegate(storage, slot, to, amount);

            response.push_main_ica_msg(msg);
        }

        TxMsg::Undelegate(slot, amount) => {
            let msg = undelegate(storage, slot, amount);

            response.push_main_ica_msg(msg);
        }

        TxMsg::Delegate(slot, amount) => {
            let msg = delegate(storage, slot, amount);

            response.push_main_ica_msg(msg);
        }

        TxMsg::Authz(msgs) => {
            let msg = authz(storage, msgs);

            response.push_main_ica_msg(msg);
        }
    }
}

fn refund_msg(deps: Deps<NeutronQuery>, tx_count: usize) -> Result<SubMsg<NeutronMsg>> {
    let fee = query_min_ibc_fee(deps).map(|res| res.min_fee)?;

    let refund_amount = fee.total_fee_per_tx() * tx_count as u128;

    let to_address = deps
        .storage
        .reconcile_trigger_address()
        .expect("always: set when reconcile triggered");

    let msg = BankMsg::Send {
        to_address,
        amount: coins(refund_amount, IBC_FEE_DENOM),
    };

    Ok(SubMsg::new(msg))
}

fn handle_reconcile_event(storage: &mut dyn Storage, env: &CwEnv, event: Event) {
    match event {
        Event::SlashDetected(slashed_ratio) => {
            strategy::acknowledge_slashing(storage, slashed_ratio)
        }

        Event::UndelegatedAssetsTransferred => {
            let icq_update_timestamp = storage
                .last_main_ica_balance_icq_update()
                .expect("always: timestamp set on every update");

            storage.set_last_used_main_ica_balance_icq_update(icq_update_timestamp);
        }

        Event::UnbondStarted(amount) => {
            storage.set_last_unbond_timestamp(env.block.time.seconds());

            let idx = storage.unbonding_issued_count().unwrap_or_default();

            let unbonding_period = storage.unbonding_period();

            storage.set_unbonding_expected_amount(idx, amount);
            storage.set_unbonding_local_expiry(idx, env.block.time.seconds() + unbonding_period);
            storage.set_unbonding_issued_count(idx + 1);
        }

        // swap in delegations icq for the new set
        Event::RedelegationSuccessful {
            slot: ValidatorSetSlot(slot),
            validator,
        } => {
            let next_delegations_icq = storage
                .next_delegations_icq()
                .expect("always: set during redelegations");

            storage.set_delegations_icq(next_delegations_icq);

            storage.set_validator(slot, &validator);
        }

        _ => {}
    }
}

fn handle_reconcile_response(
    deps: DepsMut<NeutronQuery>,
    env: CwEnv,
    response: FsmResponse,
) -> Result<Response<NeutronMsg>> {
    for cmd in response.cmds {
        handle_reconcile_cmd(deps.storage, cmd);
    }

    for event in response.events {
        handle_reconcile_event(deps.storage, &env, event)
    }

    let Some(tx_msgs) = response.tx_msgs else {
        if response.tx_skip_count == 0 {
            return Ok(Response::default());
        }

        let refund_msg = refund_msg(deps.as_ref(), response.tx_skip_count)?;

        return Ok(Response::default().add_submessage(refund_msg));
    };

    let fee = query_min_ibc_fee(deps.as_ref()).map(|res| res.min_fee)?;

    let mut builder = ResponseBuilder::default();

    for tx_msg in tx_msgs.msgs {
        handle_reconcile_tx_msg(deps.storage, &env, &mut builder, &fee, tx_msg);
    }

    if response.tx_skip_count != 0 {
        let refund_msg = refund_msg(deps.as_ref(), response.tx_skip_count)?;

        builder.push_local_msg(refund_msg);
    }

    Ok(builder.build(deps.storage, fee))
}

pub fn reconcile_cost(deps: Deps<NeutronQuery>, phase: Phase, state: State) -> Result<Coin> {
    let storage_wrapper = StorageWrapper {
        storage: deps.storage,
    };

    let unfunded_tx_count = match state {
        State::Idle => phase.sequence_tx_count(
            state,
            storage_wrapper.validator_set_size(),
            storage_wrapper.max_msg_count(),
        ),

        State::Failed => phase.tx_count(
            storage_wrapper.validator_set_size(),
            storage_wrapper.max_msg_count(),
        ),

        State::Pending => 0,
    };

    let fee = query_min_ibc_fee(deps).map(|res| res.min_fee)?;

    let cost = fee.total_fee_per_tx() * unfunded_tx_count as u128;

    Ok(coin(cost, IBC_FEE_DENOM))
}

pub fn current_deposits(storage: &dyn Storage) -> u128 {
    let storage_wrapper = StorageWrapper { storage };
    pos_reconcile_fsm::current_deposits(&storage_wrapper)
}

fn trigger(
    deps: DepsMut<NeutronQuery>,
    env: CwEnv,
    info: MessageInfo,
    fee_recipient: Option<FeeRecipient>,
) -> Result<Response<NeutronMsg>> {
    let state = deps.storage.reconcile_state();

    if state.is_pending() {
        bail!("reconcile already in progress");
    }

    let phase = deps.storage.reconcile_phase();

    let cost = reconcile_cost(deps.as_ref(), phase, state)?;

    let payment = must_pay(&info, &cost.denom)?;

    if payment < cost.amount {
        bail!("insufficient funds for reconcile sequence: expected {} {IBC_FEE_DENOM}, received {payment} {IBC_FEE_DENOM}", cost.amount);
    }

    if let Some(recipient) = fee_recipient.as_deref() {
        deps.storage.set_fee_recipient(recipient);
    } else {
        deps.storage.clear_fee_recipient();
    }

    deps.storage
        .set_reconcile_trigger_address(info.sender.as_str());

    let storage_wrapper = StorageWrapper {
        storage: deps.storage,
    };

    let reconcile_env = Env {
        deps: deps.as_ref(),
        env: &env,
        fee_recipient,
    };

    let response = fsm(&storage_wrapper, &storage_wrapper, &reconcile_env).reconcile();

    handle_reconcile_response(deps, env, response)
}

fn success(deps: DepsMut<NeutronQuery>, env: CwEnv) -> Result<Response<NeutronMsg>> {
    let fee_recipient = deps.storage.fee_recipient();

    let storage_wrapper = StorageWrapper {
        storage: deps.storage,
    };

    let reconcile_env = Env {
        deps: deps.as_ref(),
        env: &env,
        fee_recipient,
    };

    let response = fsm(&storage_wrapper, &storage_wrapper, &reconcile_env).reconcile();

    handle_reconcile_response(deps, env, response)
}

fn failure(deps: DepsMut<NeutronQuery>, env: CwEnv) -> Result<Response<NeutronMsg>> {
    let storage_wrapper = StorageWrapper {
        storage: deps.storage,
    };

    let reconcile_env = Env {
        deps: deps.as_ref(),
        env: &env,
        fee_recipient: None,
    };

    let response = fsm(&storage_wrapper, &storage_wrapper, &reconcile_env).failed();

    handle_reconcile_response(deps, env, response)
}

pub fn reconcile(
    deps: DepsMut<NeutronQuery>,
    env: CwEnv,
    source: Source,
) -> Result<Response<NeutronMsg>> {
    match source {
        Source::Trigger(info, fee_recipient) => trigger(deps, env, info, fee_recipient),
        Source::Continuation(Status::Success) => success(deps, env),
        Source::Continuation(Status::Failure) => failure(deps, env),
    }
}

pub fn force_next(deps: DepsMut<NeutronQuery>, env: CwEnv) -> Result<Response<NeutronMsg>> {
    let fee_recipient = deps.storage.fee_recipient();

    let storage_wrapper = StorageWrapper {
        storage: deps.storage,
    };

    let reconcile_env = Env {
        deps: deps.as_ref(),
        env: &env,
        fee_recipient,
    };

    let Some(response) = fsm(&storage_wrapper, &storage_wrapper, &reconcile_env).force_next()
    else {
        bail!(
            "force next not available for phase {} in state {}",
            storage_wrapper.phase(),
            storage_wrapper.state()
        );
    };

    handle_reconcile_response(deps, env, response)
}
