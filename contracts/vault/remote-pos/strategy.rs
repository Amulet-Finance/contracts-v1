use amulet_cw::vault::{unbonding_log, UnbondingLog};
use amulet_ntrn::query::QuerierExt;
use anyhow::{bail, ensure, Result};
use bech32::{Bech32, Hrp};
use cosmwasm_std::{
    coins, BankMsg, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, Storage, Timestamp,
};

use amulet_core::{
    vault::{
        ClaimAmount, DepositAmount, DepositValue, Now as VaultNow, Strategy as CoreStrategy,
        StrategyCmd, TotalDepositsValue, UnbondEpoch, UnbondReadyStatus, UnbondingLog as _,
        UnbondingLogSet,
    },
    Asset, Decimals, Identifier,
};
use cw_utils::must_pay;
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};
use num::FixedU256;
use pos_reconcile_fsm::types::{PendingDeposit, PendingUnbond};

use crate::{
    icq,
    reconcile::current_deposits,
    state::StorageExt,
    types::{AvailableToClaim, Ica, Icq, TotalActualUnbonded, TotalExpectedUnbonded},
};

pub struct Strategy<'a> {
    storage: &'a dyn Storage,
    now: Timestamp,
}

impl<'a> Strategy<'a> {
    pub fn new(storage: &'a dyn Storage, env: &Env) -> Self {
        Self {
            storage,
            now: env.block.time,
        }
    }
}

fn unbond_ready(storage: &dyn Storage, now: Timestamp, unbond_amount: u128) -> UnbondReadyStatus {
    let unbonding_period = storage.unbonding_period();

    let estimated_block_time = storage.estimated_block_interval_seconds();

    let fee_payment_cooldown_blocks = storage.fee_payment_cooldown_blocks();

    let buffer_period = (fee_payment_cooldown_blocks * 3) * estimated_block_time;

    let pending_batch_slashed_amount = storage.pending_batch_slashed_amount();

    let amount = unbond_amount
        .checked_sub(pending_batch_slashed_amount)
        .map(ClaimAmount)
        .expect("always: pending batch slashed amount <= pending batch amount");

    let epoch = UnbondEpoch {
        start: now.seconds(),
        end: now.seconds() + unbonding_period + buffer_period,
    };

    UnbondReadyStatus::Ready { amount, epoch }
}

impl<'a> CoreStrategy for Strategy<'a> {
    fn now(&self) -> VaultNow {
        self.now.seconds()
    }

    fn deposit_asset(&self) -> Asset {
        self.storage.ibc_deposit_asset().into()
    }

    fn underlying_asset_decimals(&self) -> Decimals {
        self.storage.remote_denom_decimals()
    }

    fn total_deposits_value(&self) -> TotalDepositsValue {
        let total_deposits_value = current_deposits(self.storage);
        TotalDepositsValue(total_deposits_value)
    }

    fn deposit_value(&self, DepositAmount(amount): DepositAmount) -> DepositValue {
        // 1:1 (no exchange/redemption rate)
        DepositValue(amount)
    }

    fn unbond(&self, DepositValue(unbond_amount): DepositValue) -> UnbondReadyStatus {
        if self.storage.reconcile_state().is_pending() {
            return UnbondReadyStatus::Later(None);
        }

        let Some(last_unbond_timestamp) = self.storage.last_unbond_timestamp() else {
            return unbond_ready(self.storage, self.now, unbond_amount);
        };

        let minimun_unbond_interval = self.storage.minimum_unbond_interval();

        let elapsed = self.now.seconds() - last_unbond_timestamp;

        if elapsed < minimun_unbond_interval {
            return UnbondReadyStatus::Later(Some(elapsed.abs_diff(minimun_unbond_interval)));
        }

        let PendingUnbond(pending_unbond) = self.storage.pending_unbond();

        // Only ready to submit the batch once the previous one has been unbonded.
        if pending_unbond > 0 {
            return UnbondReadyStatus::Later(None);
        }

        unbond_ready(self.storage, self.now, unbond_amount)
    }
}

fn send_claimed_unbondings<C>(
    storage: &mut dyn Storage,
    amount: u128,
    recipient: Identifier,
) -> Result<CosmosMsg<C>> {
    let TotalActualUnbonded(total_actual_unbonded) = storage.total_actual_unbonded();

    let TotalExpectedUnbonded(total_expected_unbonded) = storage.total_expected_unbonded();

    let AvailableToClaim(available_to_claim) = storage.available_to_claim();

    ensure!(
        total_actual_unbonded > 0,
        "total_actual_unbondings > 0 for claims to be processed"
    );

    ensure!(
        total_expected_unbonded > 0,
        "total_expected_unbondings > 0 for claims to be processed"
    );

    let numer = FixedU256::from_u128(total_actual_unbonded.min(total_expected_unbonded));

    let denom = FixedU256::from_u128(total_expected_unbonded);

    let ratio = numer
        .checked_div(denom)
        .expect("checked: total expected unbonded is not zero");

    let claim_amount = ratio
        .checked_mul(FixedU256::from_u128(amount))
        .expect("always: ratio <= 1.0")
        .floor();

    if available_to_claim < claim_amount {
        bail!("insufficient amount of unbondings received: {available_to_claim} < {claim_amount}");
    }

    let available_to_claim = available_to_claim
        .checked_sub(claim_amount)
        .expect("checked: claim amount <= available to claim");

    storage.set_available_to_claim(AvailableToClaim(available_to_claim));

    let ibc_deposit_asset = storage.ibc_deposit_asset();

    Ok(BankMsg::Send {
        to_address: recipient.into_string(),
        amount: coins(claim_amount, ibc_deposit_asset),
    }
    .into())
}

pub fn handle_cmd<C>(storage: &mut dyn Storage, cmd: StrategyCmd) -> Result<Option<CosmosMsg<C>>> {
    match cmd {
        StrategyCmd::Deposit { amount } => {
            let PendingDeposit(pending_deposit) = storage.pending_deposit();

            let pending_deposit = pending_deposit
                .checked_add(amount.0)
                .expect("pending deposit will not overflow 128 bits");

            storage.set_pending_deposit(PendingDeposit(pending_deposit));
        }

        StrategyCmd::Unbond { value } => {
            let PendingUnbond(pending_unbond) = storage.pending_unbond();

            let pending_unbond = pending_unbond
                .checked_add(value.0)
                .expect("pending unbond will not overflow 128 bits");

            storage.set_pending_unbond(PendingUnbond(pending_unbond));
            // clear pending batch slashed amount now there is a new pending batch
            storage.set_pending_batch_slashed_amount(0);
        }

        StrategyCmd::SendClaimed { amount, recipient } => {
            return send_claimed_unbondings(storage, amount.0, recipient).map(Some)
        }
    }

    Ok(None)
}

// Mirrors: https://github.com/neutron-org/neutron/blob/v2.0.0/x/ibc-hooks/utils/utils.go#L68
fn ica_ibc_hook_address(channel: &str, ica_address: &str) -> String {
    const HOOK_ADDR_PREFIX: &str = "ibc-wasm-hook-intermediary";
    const BECH32_PREFIX: &str = "neutron";

    // first hash the 'type' prefix as per:
    // https://github.com/cosmos/cosmos-sdk/blob/v0.47.6/types/address/hash.go#L26
    let header = hmac_sha256::Hash::hash(HOOK_ADDR_PREFIX.as_bytes());

    let mut hasher = hmac_sha256::Hash::new();

    // Add the hashed 'type' prefix 'header' to a fresh hasher
    hasher.update(header);
    // then add '<channel>/<address>'
    hasher.update(channel);
    hasher.update("/");
    hasher.update(ica_address);

    let address_bytes = hasher.finalize();

    let hrp = Hrp::parse(BECH32_PREFIX).expect("valid prefix");

    bech32::encode::<Bech32>(hrp, &address_bytes)
        .expect("infallible bech32 encoding")
        .to_string()
}

fn increase_pending_batch_slashed_amount(storage: &mut dyn Storage, slashed_ratio: FixedU256) {
    let unbonding_log = UnbondingLog::new(storage);

    let pending_batch_slashed_amount = storage.pending_batch_slashed_amount();

    let pending_batch_id = unbonding_log
        .last_committed_batch_id()
        .map_or(0, |id| id + 1);

    let DepositValue(pending_batch_unbond_value) = unbonding_log
        .batch_unbond_value(pending_batch_id)
        .unwrap_or_default();

    if pending_batch_unbond_value == 0 {
        return;
    }

    let increase = pending_batch_unbond_value
        .checked_sub(pending_batch_slashed_amount)
        .map(FixedU256::from_u128)
        .expect("always: pending batch slashed amount <= pending batch amount")
        .checked_mul(slashed_ratio)
        .expect("always: slashed ratio <= 1.0")
        .floor();

    let pending_batch_slashed_amount = pending_batch_slashed_amount
        .checked_add(increase)
        .expect("always: pending batch slashed amount <= pending batch value <= u128::MAX");

    storage.set_pending_batch_slashed_amount(pending_batch_slashed_amount);
}

fn slash_last_committed_batch(storage: &mut dyn Storage, slashed_ratio: FixedU256) {
    let unbonding_log = UnbondingLog::new(storage);

    let Some(last_committed_batch_id) = unbonding_log.last_committed_batch_id() else {
        return;
    };

    let ClaimAmount(claimable_amount) = unbonding_log
        .batch_claimable_amount(last_committed_batch_id)
        .expect("always: committed batches have claimable amounts set");

    let slashed_amount = FixedU256::from_u128(claimable_amount)
        .checked_mul(slashed_ratio)
        .expect("always: slashed ratio <= 1.0")
        .floor();

    unbonding_log::handle_cmd(
        storage,
        UnbondingLogSet::BatchClaimableAmount {
            batch: last_committed_batch_id,
            amount: ClaimAmount(slashed_amount),
        },
    );
}

pub fn acknowledge_slashing(storage: &mut dyn Storage, slashed_ratio: FixedU256) {
    increase_pending_batch_slashed_amount(storage, slashed_ratio);
    slash_last_committed_batch(storage, slashed_ratio);
}

pub fn acknowledge_expected_unbondings(
    storage: &mut dyn Storage,
    balance_icq_timestamp: u64,
) -> u128 {
    let issued_count = storage.unbonding_issued_count().unwrap_or_default();

    let mut ack_count = storage.unbonding_ack_count().unwrap_or_default();

    let mut ack_amount = 0u128;

    while ack_count != issued_count {
        let local_expiry = storage
            .unbonding_local_expiry(ack_count)
            .expect("always: unbonding record exists");

        if local_expiry >= balance_icq_timestamp {
            break;
        }

        let expected_amount = storage
            .unbonding_expected_amount(ack_count)
            .expect("always: unbonding record exists");

        ack_amount = ack_amount
            .checked_add(expected_amount)
            .expect("ack amount will never over 128 bits");

        ack_count += 1;
    }

    storage.set_unbonding_ack_count(ack_count);

    ack_amount
}

pub fn handle_receive_unbonded(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    balance_icq_timestamp: u64,
) -> Result<Response<NeutronMsg>> {
    let transfer_out_channel = deps.storage.transfer_out_channel();

    let ica_address = deps
        .storage
        .main_ica_address()
        .expect("must have a main ICA in order to receive unbonded assets from it");

    let expected_hook_sender = ica_ibc_hook_address(&transfer_out_channel, &ica_address);

    if info.sender != expected_hook_sender {
        bail!(
            "invalid hook sender: sender is '{}', expected '{expected_hook_sender}'",
            info.sender
        )
    }

    let ibc_denom = deps.storage.ibc_deposit_asset();

    let unbondings_received = cw_utils::must_pay(&info, ibc_denom.as_str())?;

    let unbondings_expected = acknowledge_expected_unbondings(deps.storage, balance_icq_timestamp);

    let TotalActualUnbonded(total_actual_unbonded) = deps.storage.total_actual_unbonded();

    let TotalExpectedUnbonded(total_expected_unbonded) = deps.storage.total_expected_unbonded();

    let AvailableToClaim(available_to_claim) = deps.storage.available_to_claim();

    let total_actual_unbonded = total_actual_unbonded
        .checked_add(unbondings_received.u128())
        .expect("total actual unbonded should never overflow 128 bits");

    let total_expected_unbonded = total_expected_unbonded
        .checked_add(unbondings_expected)
        .expect("total expected unbonded should never overflow 128 bits");

    let available_to_claim = available_to_claim
        .checked_add(unbondings_received.u128())
        .expect("available to claim should never overflow 128 bits");

    deps.storage
        .set_total_actual_unbonded(TotalActualUnbonded(total_actual_unbonded));

    deps.storage
        .set_total_expected_unbonded(TotalExpectedUnbonded(total_expected_unbonded));

    deps.storage
        .set_available_to_claim(AvailableToClaim(available_to_claim));

    Ok(Response::default())
}

fn must_pay_icq_deposit(deps: Deps<NeutronQuery>, info: &MessageInfo) -> Result<()> {
    let icq_deposit = deps.querier.interchain_query_deposit()?;

    let sender_deposit = must_pay(info, &icq_deposit.denom)?;

    if sender_deposit != icq_deposit.amount {
        bail!(
            "insufficient ICQ deposit: received '{sender_deposit}{denom}', expected '{expected_deposit}{denom}'",
            expected_deposit = icq_deposit.amount,
            denom = icq_deposit.denom,
        )
    }

    Ok(())
}

pub fn handle_redelegate_slot(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    slot: usize,
    validator: String,
) -> Result<Response<NeutronMsg>> {
    if deps.storage.redelegate_slot().is_some() {
        bail!("another redelegation is pending");
    }

    must_pay_icq_deposit(deps.as_ref(), &info)?;

    let mut validators = deps.storage.validators();

    let Some(slot_validator) = validators.get_mut(slot) else {
        bail!("invalid slot index");
    };

    deps.storage.set_redelegate_slot(slot);
    deps.storage.set_redelegate_to(&validator);

    *slot_validator = validator;

    let msg = icq::main_ica_next_delegations_registration_msg(deps.storage, validators);

    Ok(Response::default().add_submessage(msg))
}

pub fn handle_restore_ica(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    ica: Ica,
) -> Result<Response<NeutronMsg>> {
    let ica_register_fee = deps.querier.interchain_account_register_fee()?;

    let sender_fee = must_pay(&info, &ica_register_fee.denom)?;

    if sender_fee != ica_register_fee.amount {
        bail!(
            "insufficient ICA register fee: received '{sender_fee}{denom}', expected '{expected_fee}{denom}'",
            expected_fee = ica_register_fee.amount,
            denom = ica_register_fee.denom,
        )
    }

    let connection_id = deps.storage.connection_id();

    let msg = NeutronMsg::RegisterInterchainAccount {
        connection_id,
        interchain_account_id: ica.id().to_owned(),
        register_fee: Some(vec![ica_register_fee]),
    };

    Ok(Response::default().add_message(msg))
}

pub fn handle_restore_icq(
    deps: DepsMut<NeutronQuery>,
    info: MessageInfo,
    icq: Icq,
) -> Result<Response<NeutronMsg>> {
    must_pay_icq_deposit(deps.as_ref(), &info)?;

    let msg = match icq {
        Icq::MainBalance => icq::ica_balance_registration_msg(deps.storage, Ica::Main),
        Icq::RewardsBalance => icq::ica_balance_registration_msg(deps.storage, Ica::Rewards),
        Icq::MainDelegations => icq::main_ica_current_delegations_registration_msg(
            deps.storage,
            deps.storage.validators(),
        ),
    };

    Ok(Response::default().add_submessage(msg))
}
