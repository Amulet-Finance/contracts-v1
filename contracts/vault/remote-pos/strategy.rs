use amulet_ntrn::query::QuerierExt;
use anyhow::{bail, Result};
use bech32::{Bech32, Hrp};
use cosmwasm_std::{
    coins, BankMsg, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, Storage, Timestamp,
};

use amulet_core::{
    vault::{
        DepositAmount, DepositValue, Now as VaultNow, Strategy as CoreStrategy, StrategyCmd,
        TotalDepositsValue, UnbondEpoch, UnbondReadyStatus,
    },
    Asset, Decimals,
};
use cw_utils::must_pay;
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};
use num::FixedU256;
use pos_reconcile_fsm::types::{Delegated, PendingDeposit, PendingUnbond};

use crate::{
    icq,
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
        // Delegated + Pending Deposits - Pending Unbond
        let Delegated(delegated) = self.storage.delegated();
        let PendingDeposit(pending_deposit) = self.storage.pending_deposit();
        let PendingUnbond(pending_unbond) = self.storage.pending_unbond();

        delegated
            .checked_add(pending_deposit)
            .expect("adding pending deposit value will not overflow 128 bits")
            .checked_sub(pending_unbond)
            .expect("always: pending unbond <= delegated")
    }

    fn deposit_value(&self, amount: DepositAmount) -> DepositValue {
        // 1:1 (no exchange/redemption rate)
        amount
    }

    fn unbond(&self, value: DepositValue) -> UnbondReadyStatus {
        if self.storage.reconcile_state().is_pending() {
            return UnbondReadyStatus::Later(None);
        }

        let unbond_ready = || {
            let unbonding_period = self.storage.unbonding_period();

            let estimated_block_time = self.storage.estimated_block_interval_seconds();

            let fee_payment_cooldown_blocks = self.storage.fee_payment_cooldown_blocks();

            let buffer_period = (fee_payment_cooldown_blocks * 3) * estimated_block_time;

            let epoch = UnbondEpoch {
                start: self.now.seconds(),
                end: self.now.seconds() + unbonding_period + buffer_period,
            };

            UnbondReadyStatus::Ready {
                amount: value,
                epoch,
            }
        };

        let Some(last_unbond_timestamp) = self.storage.last_unbond_timestamp() else {
            return unbond_ready();
        };

        let minimun_unbond_interval = self.storage.minimum_unbond_interval();

        let elapsed = self.now.seconds() - last_unbond_timestamp;

        if elapsed < minimun_unbond_interval {
            return UnbondReadyStatus::Later(Some(elapsed.abs_diff(minimun_unbond_interval)));
        }

        unbond_ready()
    }
}

pub fn handle_cmd<CustomMsg>(
    storage: &mut dyn Storage,
    cmd: StrategyCmd,
) -> Option<CosmosMsg<CustomMsg>> {
    match cmd {
        StrategyCmd::Deposit { amount } => {
            let PendingDeposit(pending_deposit) = storage.pending_deposit();

            let pending_deposit = pending_deposit
                .checked_add(amount)
                .expect("pending deposit will not overflow 128 bits");

            storage.set_pending_deposit(PendingDeposit(pending_deposit));
        }

        StrategyCmd::Unbond { value } => {
            let PendingUnbond(pending_unbond) = storage.pending_unbond();

            let pending_unbond = pending_unbond
                .checked_add(value)
                .expect("pending unbond will not overflow 128 bits");

            storage.set_pending_unbond(PendingUnbond(pending_unbond));
        }

        StrategyCmd::SendClaimed { amount, recipient } => {
            let TotalActualUnbonded(total_actual_unbonded) = storage.total_actual_unbonded();

            let TotalExpectedUnbonded(total_expected_unbonded) = storage.total_expected_unbonded();

            let AvailableToClaim(available_to_claim) = storage.available_to_claim();

            let numer = FixedU256::from_u128(total_actual_unbonded.min(total_expected_unbonded));

            let denom = FixedU256::from_u128(total_expected_unbonded);

            let ratio = numer
                .checked_div(denom)
                .expect("total expected unbonded is not zero");

            let claim_amount = ratio
                .checked_mul(FixedU256::from_u128(amount))
                .expect("always: ratio <= 1.0")
                .floor()
                .min(available_to_claim);

            let available_to_claim = available_to_claim
                .checked_sub(claim_amount)
                .expect("always: claim amount <= available to claim");

            storage.set_available_to_claim(AvailableToClaim(available_to_claim));

            let ibc_deposit_asset = storage.ibc_deposit_asset();

            return Some(
                BankMsg::Send {
                    to_address: recipient.into_string(),
                    amount: coins(claim_amount, ibc_deposit_asset),
                }
                .into(),
            );
        }
    }

    None
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

pub fn acknowledge_expected_unbondings(storage: &mut dyn Storage) -> u128 {
    let issued_count = storage.unbonding_issued_count().unwrap_or_default();

    let last_used_icq_timestamp = storage
        .last_used_main_ica_balance_icq_update()
        .expect("always: must have been set to receive undelegated assets");

    let mut ack_count = storage.unbonding_ack_count().unwrap_or_default();

    let mut ack_amount = 0u128;

    while ack_count != issued_count {
        let local_expiry = storage
            .unbonding_local_expiry(ack_count)
            .expect("always: unbonding record exists");

        if local_expiry >= last_used_icq_timestamp {
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

    let unbondings_expected = acknowledge_expected_unbondings(deps.storage);

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
