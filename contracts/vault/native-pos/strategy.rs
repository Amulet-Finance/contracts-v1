use amulet_cw::vault::UnbondingLog;
use anyhow::{anyhow, bail, ensure, Result};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, to_json_binary, Attribute, BankMsg, Coin, CosmosMsg, DelegationTotalRewardsResponse,
    DistributionMsg, Env, QuerierWrapper, Response, StakingMsg, Storage, Uint128, WasmMsg,
};

use amulet_core::{
    admin::AdminRole,
    vault::{
        BatchId, ClaimAmount, DepositAmount, DepositValue, Now as VaultNow,
        Strategy as CoreStrategy, StrategyCmd, TotalDepositsValue, UnbondEpoch, UnbondReadyStatus,
        UnbondingLog as CoreUnbondingLog,
    },
    Asset, Decimals, Identifier,
};

use crate::state::{StorageExt, ValidatorSet};

pub struct Strategy<'a> {
    storage: &'a dyn Storage,
    querier: QuerierWrapper<'a>,
    env: &'a Env,
}

impl<'a> Strategy<'a> {
    pub fn new(storage: &'a dyn Storage, querier: QuerierWrapper<'a>, env: &'a Env) -> Self {
        Self {
            storage,
            querier,
            env,
        }
    }
}

type ValidatorSetDelegations = rustc_hash::FxHashMap<String, u128>;

fn query_all_delegations(querier: QuerierWrapper, env: &Env) -> Result<ValidatorSetDelegations> {
    let delegations = querier
        .query_all_delegations(&env.contract.address)?
        .into_iter()
        .map(|d| (d.validator, d.amount.amount.u128()))
        .collect();

    Ok(delegations)
}

impl Strategy<'_> {
    fn total_actual_delegations(&self) -> u128 {
        query_all_delegations(self.querier, self.env)
            .expect("delegations query for contract address should not fail")
            .into_values()
            .reduce(|total, delegated_amount| {
                total
                    .checked_add(delegated_amount)
                    .expect("total delegated amount should never overflow")
            })
            // if there are no delegations yet, return 0
            .unwrap_or_default()
    }

    pub fn is_slashed(&self) -> bool {
        self.total_actual_delegations() < self.storage.delegated()
    }
}

impl CoreStrategy for Strategy<'_> {
    fn now(&self) -> VaultNow {
        self.env.block.time.seconds()
    }

    fn deposit_asset(&self) -> Asset {
        self.storage.bond_denom().into()
    }

    fn underlying_asset_decimals(&self) -> Decimals {
        self.storage.bond_denom_decimals()
    }

    fn total_deposits_value(&self) -> TotalDepositsValue {
        TotalDepositsValue(self.total_actual_delegations())
    }

    fn deposit_value(&self, DepositAmount(amount): DepositAmount) -> DepositValue {
        // 1:1 (e.g. no exchange/redemption rate like an LST)
        DepositValue(amount)
    }

    fn unbond(&self, DepositValue(unbond_amount): DepositValue) -> UnbondReadyStatus {
        let last_unbond_timestamp = self.storage.last_unbond_timestamp();

        let minimum_unbond_interval = self.storage.minimum_unbond_interval();

        let elapsed = self.now() - last_unbond_timestamp;

        if elapsed < minimum_unbond_interval {
            return UnbondReadyStatus::Later(Some(last_unbond_timestamp + minimum_unbond_interval));
        }

        UnbondReadyStatus::Ready {
            amount: ClaimAmount(unbond_amount),
            epoch: UnbondEpoch {
                start: self.now(),
                end: self.now() + self.storage.unbonding_period(),
            },
        }
    }
}

fn distribute_delegations(
    validators: &ValidatorSet,
    amount: u128,
) -> impl Iterator<Item = (&str, u128)> {
    let base_amount = amount / validators.len() as u128;

    let remainder = amount % validators.len() as u128;

    validators.iter().enumerate().map(move |(idx, validator)| {
        let delegation = if (idx as u128) < remainder {
            base_amount + 1
        } else {
            base_amount
        };
        (validator.as_str(), delegation)
    })
}

fn delegate_msgs(validators: ValidatorSet, bond_denom: &str, amount: u128) -> Vec<CosmosMsg> {
    distribute_delegations(&validators, amount)
        .map(|(validator, amount)| {
            StakingMsg::Delegate {
                validator: validator.to_owned(),
                amount: coin(amount, bond_denom),
            }
            .into()
        })
        .collect()
}

fn deposit(
    storage: &mut dyn Storage,
    DepositAmount(amount): DepositAmount,
) -> Result<Vec<CosmosMsg>> {
    let delegated = storage
        .delegated()
        .checked_add(amount)
        .expect("increasing delegated amount by deposit amount should never overflow");

    storage.set_delegated(delegated);

    let msgs = delegate_msgs(storage.validators(), &storage.bond_denom(), amount);

    Ok(msgs)
}

fn distribute_undelegation(
    delegations: &[(String, u128)],
    amount: u128,
) -> Result<Vec<(&str, u128)>> {
    if amount == 0 {
        return Ok(vec![]);
    }

    let total_delegations: u128 = delegations.iter().map(|(_, amount)| *amount).sum();

    ensure!(
        total_delegations >= amount,
        "insufficient active set staked balance to satisfy undelegation: {} > {}",
        amount,
        total_delegations
    );

    let mut undelegations = Vec::with_capacity(delegations.len());
    let mut total_distibuted = 0u128;

    for (validator, delegation) in delegations {
        let undelegate_amount = Uint128::new(amount)
            .multiply_ratio(*delegation, total_delegations)
            .u128();

        undelegations.push((validator.as_str(), undelegate_amount));
        total_distibuted += undelegate_amount;
    }

    // distribute any remainder from rounding errors
    if total_distibuted < amount {
        let mut remainder = amount - total_distibuted;

        for ((_, delegated), (_, undelegation)) in delegations.iter().zip(undelegations.iter_mut())
        {
            let extra_undelegation = remainder.min(*delegated - *undelegation);
            *undelegation += extra_undelegation;
            remainder -= extra_undelegation;
            if remainder == 0 {
                break;
            }
        }
    }

    Ok(undelegations)
}

struct UndelegateMsgsResponse<'a> {
    spent_validators_pending_redelegation: Vec<&'a str>,
    msgs: Vec<CosmosMsg>,
}

fn undelegate_msgs<'a>(
    validators_pending_redelegation: &'a ValidatorSet,
    all_delegations: ValidatorSetDelegations,
    bond_denom: &str,
    mut amount: u128,
) -> Result<UndelegateMsgsResponse<'a>> {
    let mut spent_validators_pending_redelegation = vec![];
    let mut msgs = vec![];

    for validator in validators_pending_redelegation {
        let delegation = all_delegations
            .get(validator)
            .copied()
            .expect("validator pending redelegation has non-zero delegation");

        if delegation > amount {
            msgs.push(
                StakingMsg::Undelegate {
                    validator: validator.clone(),
                    amount: coin(amount, bond_denom),
                }
                .into(),
            );

            // undelegation fulfilled
            return Ok(UndelegateMsgsResponse {
                spent_validators_pending_redelegation,
                msgs,
            });
        }

        amount = amount
            .checked_sub(delegation)
            .expect("delegation amount <= amount");

        msgs.push(
            StakingMsg::Undelegate {
                validator: validator.clone(),
                amount: coin(delegation, bond_denom),
            }
            .into(),
        );

        // remove validator from pending redelgation set now it has been full undelegated from instead
        spent_validators_pending_redelegation.push(validator.as_str());

        if amount == 0 {
            // undelegation fulfilled
            return Ok(UndelegateMsgsResponse {
                spent_validators_pending_redelegation,
                msgs,
            });
        }
    }

    let active_set_delegations: Vec<(String, u128)> = all_delegations
        .into_iter()
        .filter_map(|(validator, amount)| {
            (!validators_pending_redelegation.contains(&validator)).then_some((validator, amount))
        })
        .collect();

    for (validator, amount) in distribute_undelegation(&active_set_delegations, amount)? {
        if amount == 0 {
            continue;
        }

        msgs.push(
            StakingMsg::Undelegate {
                validator: validator.to_owned(),
                amount: coin(amount, bond_denom),
            }
            .into(),
        );
    }

    Ok(UndelegateMsgsResponse {
        spent_validators_pending_redelegation,
        msgs,
    })
}

fn unbond(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
    DepositValue(amount): DepositValue,
) -> Result<Vec<CosmosMsg>> {
    let delegated = storage
        .delegated()
        .checked_sub(amount)
        .expect("decreasing delegated amount by unbond amount should never overflow");

    storage.set_delegated(delegated);

    let validators_pending_redelegation = storage.validators_pending_redelegation();

    let UndelegateMsgsResponse {
        spent_validators_pending_redelegation,
        msgs,
    } = undelegate_msgs(
        &validators_pending_redelegation,
        query_all_delegations(querier, env)?,
        &storage.bond_denom(),
        amount,
    )?;

    for spent_validator in spent_validators_pending_redelegation {
        let slot_idx = storage
            .validator_pending_redelegation_slot(spent_validator)
            .expect("spent validator is in validators pending redelegation set");
        storage.remove_validator_pending_redelegation(slot_idx);
    }

    storage.set_last_unbond_timestamp(env.block.time.seconds());

    Ok(msgs)
}

fn completed_batches(storage: &dyn Storage, env: &Env) -> Vec<(BatchId, ClaimAmount)> {
    let unbonding_log = UnbondingLog::new(storage);

    let mut batch_id = storage.last_acknowledged_batch().map_or(0, |b| b + 1);

    let mut completed_batches = vec![];

    loop {
        let Some(batch_epoch) = unbonding_log.committed_batch_epoch(batch_id) else {
            return completed_batches;
        };

        if batch_epoch.end > env.block.time.seconds() {
            return completed_batches;
        }

        completed_batches.push((
            batch_id,
            unbonding_log
                .batch_claimable_amount(batch_id)
                .expect("every committed batch has a claimable amount"),
        ));

        batch_id += 1
    }
}

fn acknowledge_completed_batches(
    storage: &mut dyn Storage,
    env: &Env,
    current_balance: u128,
) -> Result<Option<u64>> {
    let completed_batches = completed_batches(storage, env);

    if completed_batches.is_empty() {
        return Ok(storage.last_acknowledged_batch());
    }

    let last_completed_batch_id = completed_batches
        .last()
        .map(|(id, _)| *id)
        .expect("completed batches length > 0");

    storage.set_last_acknowledged_batch(last_completed_batch_id);

    let total_expected: u128 = completed_batches
        .iter()
        .map(|(_, ClaimAmount(amount))| amount)
        .sum();

    // ensure that received unbondings pending claim are not double counted
    let total_received = current_balance
        .checked_sub(storage.available_to_claim())
        .expect("balance always >= available to claim");

    storage.set_available_to_claim(current_balance);

    // no claimable adjustment required
    if total_expected <= total_received {
        return Ok(Some(last_completed_batch_id));
    }

    for (batch, ClaimAmount(expected_claimable_amount)) in completed_batches {
        let adjusted_claimbale_amount = Uint128::new(total_received)
            .multiply_ratio(expected_claimable_amount, total_expected)
            .u128();

        storage.set_batch_adjusted_claimable(batch, adjusted_claimbale_amount);
    }

    Ok(Some(last_completed_batch_id))
}

struct ClaimedBatch {
    id: BatchId,
    /// The total amount expected to be unbonded in the batch
    total_expected_unbond: u128,
    /// The amount the recipient expects to claim from the batch
    expected_claim_amount: u128,
}

fn batches_being_claimed(
    storage: &dyn Storage,
    env: &Env,
    recipient: &str,
    prev_last_claimed_batch: Option<BatchId>,
) -> Vec<ClaimedBatch> {
    let unbonding_log = UnbondingLog::new(storage);

    let Some(mut batch_id) = prev_last_claimed_batch.map_or_else(
        || unbonding_log.first_entered_batch(recipient),
        |prev| unbonding_log.next_entered_batch(recipient, prev),
    ) else {
        return vec![];
    };

    let mut batches_being_claimed = vec![];

    loop {
        let Some(epoch) = unbonding_log.committed_batch_epoch(batch_id) else {
            return batches_being_claimed;
        };

        if epoch.end > env.block.time.seconds() {
            return batches_being_claimed;
        }

        let total_expected_unbond = unbonding_log
            .batch_claimable_amount(batch_id)
            .expect("all committed batches have a claimable amount")
            .0;

        let expected_claim_amount = unbonding_log
            .unbonded_value_in_batch(recipient, batch_id)
            .expect("recipient always has unbonded value set for entered batch")
            .0;

        batches_being_claimed.push(ClaimedBatch {
            id: batch_id,
            total_expected_unbond,
            expected_claim_amount,
        });

        let Some(next_batch_id) = unbonding_log.next_entered_batch(recipient, batch_id) else {
            return batches_being_claimed;
        };

        batch_id = next_batch_id
    }
}

fn send_claimed(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
    ClaimAmount(total_expected_claim): ClaimAmount,
    recipient: Identifier,
    prev_last_claimed_batch: Option<BatchId>,
    attributes: &mut Vec<Attribute>,
) -> Result<Vec<CosmosMsg>> {
    let bond_denom = storage.bond_denom();

    let current_balance = querier.query_balance(env.contract.address.as_str(), &bond_denom)?;

    acknowledge_completed_batches(storage, env, current_balance.amount.u128())?;

    let batches_being_claimed =
        batches_being_claimed(storage, env, recipient.as_str(), prev_last_claimed_batch);

    assert!(
        !batches_being_claimed.is_empty(),
        "{recipient} must be claiming at least one batch for send claimed to be issued"
    );

    let mut total_actual_claim = Uint128::zero();

    for ClaimedBatch {
        id,
        total_expected_unbond,
        expected_claim_amount,
    } in batches_being_claimed
    {
        match storage.batch_adjusted_claimable(id) {
            Some(total_actual_unbond) => {
                let actual_claim_amount = Uint128::new(total_actual_unbond)
                    .multiply_ratio(expected_claim_amount, total_expected_unbond);
                total_actual_claim += actual_claim_amount;
            }
            None => total_actual_claim += Uint128::new(expected_claim_amount),
        }
    }

    assert!(
        total_actual_claim.u128() <= total_expected_claim,
        "total_actual_claim should never be > total_expected_claim"
    );

    let available_to_claim = storage.available_to_claim();

    if total_actual_claim.u128() > storage.available_to_claim() {
        bail!("claimable amount is greater than received unbondings: {total_actual_claim} > {available_to_claim}")
    }

    storage.set_available_to_claim(available_to_claim - total_actual_claim.u128());

    let slashed_amount = total_expected_claim.abs_diff(total_actual_claim.u128());

    attributes.push(Attribute::new(
        "claim_slashed_amount",
        slashed_amount.to_string(),
    ));

    let send_msg = BankMsg::Send {
        to_address: recipient.into_string(),
        amount: vec![Coin {
            denom: bond_denom,
            amount: total_actual_claim,
        }],
    }
    .into();

    Ok(vec![send_msg])
}

pub fn handle_cmd(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
    cmd: StrategyCmd,
    prev_last_claimed_batch: Option<BatchId>,
    attributes: &mut Vec<Attribute>,
) -> Result<Vec<CosmosMsg>> {
    match cmd {
        StrategyCmd::Deposit { amount } => deposit(storage, amount),

        StrategyCmd::Unbond { value } => unbond(storage, querier, env, value),

        StrategyCmd::SendClaimed { amount, recipient } => send_claimed(
            storage,
            querier,
            env,
            amount,
            recipient,
            prev_last_claimed_batch,
            attributes,
        ),
    }
}

fn query_rewards_sink_balance(
    querier: QuerierWrapper,
    reward_sink_address: &str,
    bond_denom: &str,
) -> Result<u128> {
    let balance = querier.query_balance(reward_sink_address, bond_denom)?;

    Ok(balance.amount.u128())
}

pub fn handle_collect_rewards(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
) -> Result<Response> {
    let DelegationTotalRewardsResponse { rewards, total, .. } =
        querier.query_delegation_total_rewards(env.contract.address.as_str())?;

    let rewards_sink_address = storage.rewards_sink_address();

    let bond_denom = storage.bond_denom();

    let rewards_sink_balance =
        query_rewards_sink_balance(querier, &rewards_sink_address, &bond_denom)?;

    let total_pending_rewards: Uint128 = total
        .first()
        .map(|t| t.amount.to_uint_floor().try_into())
        .transpose()?
        .ok_or_else(|| anyhow!("no total delegation rewards"))?;

    let total_rewards_receivable = total_pending_rewards
        .u128()
        .checked_add(rewards_sink_balance)
        .expect("total receiveable rewards should never overflow");

    let delegated = storage
        .delegated()
        .checked_add(total_rewards_receivable)
        .expect("delegated + total receivable rewards should never overflow");

    storage.set_delegated(delegated);

    let withdraw_rewards_msgs =
        rewards
            .into_iter()
            .map(|r| DistributionMsg::WithdrawDelegatorReward {
                validator: r.validator_address,
            });

    #[cw_serde]
    struct CollectRewardsMsg {}

    let collect_rewards_sink_balance_msg = WasmMsg::Execute {
        contract_addr: rewards_sink_address,
        msg: to_json_binary(&CollectRewardsMsg {})?,
        funds: vec![],
    };

    let delegate_msgs = delegate_msgs(storage.validators(), &bond_denom, total_rewards_receivable);

    Ok(Response::default()
        .add_messages(withdraw_rewards_msgs)
        .add_message(collect_rewards_sink_balance_msg)
        .add_messages(delegate_msgs)
        .add_attribute("action", "collect_rewards")
        .add_attribute(
            "total_rewards_receivable",
            total_rewards_receivable.to_string(),
        ))
}

pub fn handle_acknowledge_unbondings(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
) -> Result<Response> {
    let bond_denom = storage.bond_denom();

    let current_balance = querier.query_balance(env.contract.address.as_str(), &bond_denom)?;

    let last_acknowledged_batch =
        acknowledge_completed_batches(storage, env, current_balance.amount.u128())?;

    Ok(Response::default()
        .add_attribute("action", "acknowledge_unbondings")
        .add_attribute(
            "last_acknowledged_batch",
            last_acknowledged_batch
                .map_or_else(|| "none".to_owned(), |batch_id| batch_id.to_string()),
        ))
}

fn redelegate_msgs(
    validators: &ValidatorSet,
    src_validator: &str,
    bond_denom: &str,
    amount: u128,
) -> Vec<CosmosMsg> {
    distribute_delegations(validators, amount)
        .map(|(dst_validator, amount)| {
            StakingMsg::Redelegate {
                src_validator: src_validator.to_owned(),
                dst_validator: dst_validator.to_owned(),
                amount: coin(amount, bond_denom),
            }
            .into()
        })
        .collect()
}

pub fn handle_process_redelegations(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
    start: Option<usize>,
    limit: Option<usize>,
) -> Result<Response> {
    let validators_pending_redelegation = storage.validators_pending_redelegation();
    let vault_validators = storage.validators();
    let bond_denom = storage.bond_denom();

    let mut response = Response::default();

    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(validators_pending_redelegation.len());
    for src_validator in validators_pending_redelegation
        .into_iter()
        .skip(start)
        .take(limit)
    {
        let slot_idx = storage
            .validator_pending_redelegation_slot(&src_validator)
            .expect("validator is in pending redelegation slot");

        let Some(delegation) = querier.query_delegation(&env.contract.address, &src_validator)?
        else {
            storage.remove_validator_pending_redelegation(slot_idx);
            continue;
        };

        if delegation.amount == delegation.can_redelegate {
            storage.remove_validator_pending_redelegation(slot_idx);
        }

        let redelegate_msgs = redelegate_msgs(
            &vault_validators,
            &src_validator,
            &bond_denom,
            delegation.can_redelegate.amount.u128(),
        );

        response = response.add_messages(redelegate_msgs);
    }

    if response.messages.is_empty() {
        bail!("no redelegations to process")
    }

    Ok(response.add_attribute("action", "process_redelegations"))
}

pub fn handle_remove_validator(
    _: AdminRole,
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: &Env,
    validator: String,
) -> Result<Response> {
    let vault_validators = storage.validators();

    if !vault_validators.contains(&validator) {
        bail!("{validator} is not in the active vault validator set");
    }

    if vault_validators.len() == 1 {
        bail!("cannot remove the last validator in the vault validator set");
    }

    let bond_denom = storage.bond_denom();

    let slot_idx = storage
        .validator_slot(&validator)
        .expect("validator is in the active validator set");

    storage.remove_validator(slot_idx);

    let mut response = Response::default()
        .add_attribute("action", "remove_validator")
        .add_attribute("validator", &validator);

    let Some(delegation) = querier.query_delegation(&env.contract.address, &validator)? else {
        return Ok(response);
    };

    if delegation.amount != delegation.can_redelegate {
        storage.add_validator_pending_redelegation(&validator)
    }

    let redelegate_msgs = redelegate_msgs(
        &vault_validators,
        &validator,
        &bond_denom,
        delegation.can_redelegate.amount.u128(),
    );

    response = response.add_messages(redelegate_msgs);

    Ok(response)
}

pub fn handle_add_validator(
    _: AdminRole,
    storage: &mut dyn Storage,
    querier: QuerierWrapper<cosmwasm_std::Empty>,
    validator: String,
) -> Result<Response> {
    if storage.validator_slot(&validator).is_some() {
        bail!("{validator} is already in the vault validator set");
    }

    if storage
        .validator_pending_redelegation_slot(&validator)
        .is_some()
    {
        bail!("{validator} is currently in the validators pending redelegation set");
    }

    if querier.query_validator(&validator)?.is_none() {
        bail!("{validator} is not in the global active validator set");
    }

    storage.add_validator(&validator);

    Ok(Response::default()
        .add_attribute("action", "add_validator")
        .add_attribute("validator", validator))
}

pub fn handle_swap_validator(
    _: AdminRole,
    storage: &mut dyn Storage,
    querier: QuerierWrapper<cosmwasm_std::Empty>,
    env: &Env,
    from_validator: String,
    to_validator: String,
) -> Result<Response> {
    let Some(from_validator_slot) = storage.validator_slot(&from_validator) else {
        bail!("{from_validator} is not in the vault validator set");
    };

    if storage.validator_slot(&to_validator).is_some() {
        bail!("{to_validator} is already in the vault validator set");
    }

    if storage
        .validator_pending_redelegation_slot(&to_validator)
        .is_some()
    {
        bail!("{to_validator} is currently in the validators pending redelegation set");
    }

    if querier.query_validator(&to_validator)?.is_none() {
        bail!("{to_validator} is not in the global active validator set");
    }

    let response = Response::default()
        .add_attribute("action", "swap_validator")
        .add_attribute("from_validator", &from_validator)
        .add_attribute("to_validator", &to_validator);

    storage.remove_validator(from_validator_slot);
    storage.add_validator(&to_validator);

    let Some(from_validator_delegations) =
        querier.query_delegation(&env.contract.address, &from_validator)?
    else {
        // nothing to redelegate, just return a success response
        return Ok(response);
    };

    if from_validator_delegations.amount != from_validator_delegations.can_redelegate {
        storage.add_validator_pending_redelegation(&from_validator);
    }

    Ok(response.add_message(StakingMsg::Redelegate {
        src_validator: from_validator,
        dst_validator: to_validator,
        amount: from_validator_delegations.can_redelegate,
    }))
}
