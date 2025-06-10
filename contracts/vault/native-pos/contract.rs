pub mod msg;
pub mod state;
pub mod strategy;

use std::collections::HashSet;

use anyhow::{bail, ensure, Result};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, to_json_string, Binary, Deps, DepsMut,
    DistributionMsg, Env, MessageInfo, Response, SubMsg, WasmMsg,
};

use amulet_core::vault::{Cmd as VaultCmd, UnbondingLog as CoreUnbondingLog};
use amulet_cw::{
    admin::{self, get_admin_role, ExecuteMsg as AdminExecuteMsg, Repository as AdminRepository},
    vault::{
        self, handle_mint_cmd, handle_unbonding_log_cmd, init_mint_msg,
        ExecuteMsg as VaultExecuteMsg, SharesMint, UnbondingLog,
    },
    MigrateMsg,
};

use crate::{
    msg::{
        Config, ExecuteMsg, InstantiateMsg, Metadata, QueryMsg, StrategyExecuteMsg,
        StrategyQueryMsg, ValidatorSet,
    },
    state::StorageExt,
    strategy::Strategy,
};

const MAX_UNBONDING_PERIOD: u64 = 60 * 60 * 24 * 60; // two months

#[cw_serde]
pub struct RewardsSinkInitMsg {
    rewards_denom: String,
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    ensure!(
        !msg.initial_validator_set.is_empty(),
        "initial validator set must not be empty"
    );
    ensure!(
        !msg.bond_denom.is_empty(),
        "bond denom must not be an empty string"
    );
    ensure!(
        msg.unbonding_period <= MAX_UNBONDING_PERIOD,
        "unbonding period must be less than equal to {MAX_UNBONDING_PERIOD}"
    );
    ensure!(
        msg.max_unbonding_entries > 0,
        "max unbonding entries must be greater than zero"
    );

    let mut validator_hash_set = HashSet::with_capacity(msg.initial_validator_set.len());

    for validator in &msg.initial_validator_set {
        if !validator_hash_set.insert(validator) {
            bail!("validator {validator} occurs more than once in the validator set");
        }

        if deps.querier.query_validator(validator)?.is_none() {
            bail!("validator {validator} does not exist in the active set");
        }
    }

    admin::init(deps.storage, &info);

    let mininum_unbond_interval = msg.unbonding_period / msg.max_unbonding_entries;

    deps.storage.set_bond_denom(&msg.bond_denom);
    deps.storage
        .set_bond_denom_decimals(msg.bond_denom_decimals);
    deps.storage
        .set_max_unbonding_entries(msg.max_unbonding_entries);
    deps.storage
        .set_minimum_unbond_interval(mininum_unbond_interval);
    deps.storage.set_unbonding_period(msg.unbonding_period);
    deps.storage
        .set_validator_set_size(msg.initial_validator_set.len());
    // ensures first unbonding batch timer starts from now
    deps.storage
        .set_last_unbond_timestamp(env.block.time.seconds());

    for (slot, validator) in msg.initial_validator_set.iter().enumerate() {
        deps.storage.set_validator(slot, validator);
        deps.storage.set_validator_slot(validator, slot)
    }

    let code_hash_bytes = hex::decode(&msg.rewards_sink_code_hash)?;

    let reward_sink_label = "native_pos_vault_reward_sink";

    let rewards_sink_can_addr = instantiate2_address(
        &code_hash_bytes,
        &deps.api.addr_canonicalize(env.contract.address.as_str())?,
        reward_sink_label.as_bytes(),
    )?;

    let rewards_sink_addr = deps.api.addr_humanize(&rewards_sink_can_addr)?;

    deps.storage
        .set_rewards_sink_address(rewards_sink_addr.as_str());

    let init_rewards_sink_msg = WasmMsg::Instantiate2 {
        admin: None,
        code_id: msg.rewards_sink_code_id,
        label: reward_sink_label.to_owned(),
        msg: to_json_binary(&RewardsSinkInitMsg {
            rewards_denom: msg.bond_denom.clone(),
        })?,
        funds: vec![],
        salt: reward_sink_label.as_bytes().to_vec().into(),
    };

    let set_rewards_withdrawal_msg = DistributionMsg::SetWithdrawAddress {
        address: rewards_sink_addr.clone().into_string(),
    };

    deps.storage
        .set_token_factory_flavour(msg.token_factory_flavour);

    let init_mint_msg = init_mint_msg(msg.token_factory_flavour.into_factory(&env));

    Ok(Response::default()
        .add_attribute("bond_denom", msg.bond_denom)
        .add_attribute(
            "max_unbonding_entries",
            msg.max_unbonding_entries.to_string(),
        )
        .add_attribute(
            "mininum_unbond_interval",
            mininum_unbond_interval.to_string(),
        )
        .add_attribute("unbonding_period", msg.unbonding_period.to_string())
        .add_attribute("validator_set", msg.initial_validator_set.join(","))
        .add_attribute(
            "validator_set_size",
            msg.initial_validator_set.len().to_string(),
        )
        .add_attribute("rewards_sink_address", rewards_sink_addr)
        .add_message(init_rewards_sink_msg)
        .add_message(set_rewards_withdrawal_msg)
        .add_message(init_mint_msg))
}

pub fn execute_admin_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response> {
    let repository = AdminRepository::new(deps.storage);

    let cmd = admin::handle_execute_msg(deps.api, &repository, info, msg)?;

    admin::handle_cmd(deps.storage, cmd);

    Ok(Response::default())
}

pub fn execute_vault_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: VaultExecuteMsg,
) -> Result<Response> {
    let strategy = Strategy::new(deps.storage, deps.querier, &env);

    // Ensure that unbondings will not occur while in a slashed state.
    // Unbonding will be automatically re-enabled once the strategy is made whole,
    // either through donation or reward accrual.
    //
    // NOTE: The hub contract will already return a vault loss error in this case.
    // This check is to prevent non-hub senders from triggering an unbonding when
    // any pending batch has been slashed.
    if matches!(
        msg,
        VaultExecuteMsg::Redeem { .. } | VaultExecuteMsg::StartUnbond {}
    ) && strategy.is_slashed()
    {
        bail!(
            "unable to process message while in a slashed state: {}",
            to_json_string(&msg)?
        );
    }

    let unbonding_log = UnbondingLog::new(deps.storage);

    let mint = SharesMint::new(deps.storage, &env);

    // we only need to read the previous last claimed batch if the message is `Claim`
    // NOTE: we must read this value *before* any commands are executed to be sure it is
    // the previous value.
    let prev_last_claimed_batch = matches!(msg, VaultExecuteMsg::Claim {})
        .then(|| unbonding_log.last_claimed_batch(info.sender.as_str()))
        .flatten();

    let (cmds, mut response) =
        vault::handle_execute_msg(deps.api, &strategy, &unbonding_log, &mint, info, msg)?;

    for cmd in cmds {
        match cmd {
            VaultCmd::Mint(cmd) => {
                let token_factory = deps.storage.token_factory_flavour().into_factory(&env);

                let msg = handle_mint_cmd(deps.storage, token_factory, cmd);

                response.messages.push(SubMsg::new(msg));
            }

            VaultCmd::Strategy(cmd) => {
                let mut attributes = vec![];
                for msg in strategy::handle_cmd(
                    deps.storage,
                    deps.querier,
                    &env,
                    cmd,
                    prev_last_claimed_batch,
                    &mut attributes,
                )? {
                    response.messages.push(SubMsg::new(msg));
                }

                response.attributes.extend(attributes);
            }

            VaultCmd::UnbondingLog(cmd) => handle_unbonding_log_cmd(deps.storage, cmd),
        }
    }

    Ok(response)
}

pub fn execute_strategy_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: StrategyExecuteMsg,
) -> Result<Response> {
    match msg {
        StrategyExecuteMsg::CollectRewards {} => {
            strategy::handle_collect_rewards(deps.storage, deps.querier, &env)
        }

        StrategyExecuteMsg::AcknowledgeUnbondings {} => {
            strategy::handle_acknowledge_unbondings(deps.storage, deps.querier, &env)
        }

        StrategyExecuteMsg::ProcessRedelegations { start, limit } => {
            strategy::handle_process_redelegations(deps.storage, deps.querier, &env, start, limit)
        }

        StrategyExecuteMsg::RemoveValidator { validator } => {
            let repository = AdminRepository::new(deps.storage);
            let admin_role = get_admin_role(&repository, &info)?;
            strategy::handle_remove_validator(
                admin_role,
                deps.storage,
                deps.querier,
                &env,
                validator,
            )
        }

        StrategyExecuteMsg::AddValidator { validator } => {
            let repository = AdminRepository::new(deps.storage);
            let admin_role = get_admin_role(&repository, &info)?;
            strategy::handle_add_validator(admin_role, deps.storage, deps.querier, validator)
        }

        StrategyExecuteMsg::SwapValidator {
            from_validator,
            to_validator,
        } => {
            let repository = AdminRepository::new(deps.storage);
            let admin_role = get_admin_role(&repository, &info)?;
            strategy::handle_swap_validator(
                admin_role,
                deps.storage,
                deps.querier,
                &env,
                from_validator,
                to_validator,
            )
        }
    }
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => execute_admin_msg(deps, env, info, admin_msg),
        ExecuteMsg::Vault(vault_msg) => execute_vault_msg(deps, env, info, vault_msg),
        ExecuteMsg::Strategy(strategy_msg) => execute_strategy_msg(deps, env, info, strategy_msg),
    }
}

pub fn handle_strategy_query(deps: Deps, query: StrategyQueryMsg) -> Result<Binary> {
    let response = match query {
        StrategyQueryMsg::Config {} => to_json_binary(&Config {
            bond_denom: deps.storage.bond_denom(),
            bond_denom_decimals: deps.storage.bond_denom_decimals(),
            max_unbonding_entries: deps.storage.max_unbonding_entries(),
            unbonding_period: deps.storage.unbonding_period(),
        })?,

        StrategyQueryMsg::Metadata {} => to_json_binary(&Metadata {
            available_to_claim: deps.storage.available_to_claim().into(),
            delegated: deps.storage.delegated().into(),
            last_unbond_timestamp: deps.storage.last_unbond_timestamp(),
            last_acknowledged_batch: deps.storage.last_acknowledged_batch(),
            minimum_unbond_interval: deps.storage.minimum_unbond_interval(),
            rewards_sink_address: deps.storage.rewards_sink_address(),
        })?,

        StrategyQueryMsg::ValidatorSet {} => to_json_binary(&ValidatorSet {
            size: deps.storage.validator_set_size(),
            active_validators: deps.storage.validators().into_iter().collect(),
            validators_pending_redelegation: deps
                .storage
                .validators_pending_redelegation()
                .into_iter()
                .collect(),
        })?,
    };

    Ok(response)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            let repository = AdminRepository::new(deps.storage);

            admin::handle_query_msg(&repository, admin_query)?
        }

        QueryMsg::Vault(vault_query) => vault::handle_query_msg(
            deps.storage,
            &Strategy::new(deps.storage, deps.querier, &env),
            &UnbondingLog::new(deps.storage),
            &SharesMint::new(deps.storage, &env),
            &env,
            vault_query,
        )?,

        QueryMsg::Strategy(strategy_query) => handle_strategy_query(deps, strategy_query)?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response> {
    Ok(Response::default())
}
