pub mod icq;
pub mod msg;
pub mod reconcile;
pub mod reply;
pub mod state;
pub mod strategy;
pub mod sudo;
pub mod types;

use anyhow::{anyhow, bail, ensure, Result};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    SubMsg,
};
use cw_utils::must_pay;
use msg::{ReconcileState, ValidatorSet};
use neutron_sdk::{
    bindings::{msg::NeutronMsg, query::NeutronQuery},
    sudo::msg::SudoMsg,
};

use amulet_core::vault::Cmd as VaultCmd;
use amulet_cw::{
    admin::{self, ExecuteMsg as AdminExecuteMsg, Repository as AdminRepository},
    vault::{
        self, handle_mint_cmd, handle_unbonding_log_cmd, init_mint_msg,
        ExecuteMsg as VaultExecuteMsg, SharesMint, UnbondingLog,
    },
    MigrateMsg,
};
use amulet_ntrn::{query::QuerierExt as _, token_factory::TokenFactory};
use pos_reconcile_fsm::types::{Weight, Weights};
use reconcile::reconcile_cost;
use state::StorageExt;

use self::{
    msg::{
        Config, ExecuteMsg, InstantiateMsg, Metadata, QueryMsg, StrategyExecuteMsg,
        StrategyQueryMsg,
    },
    reconcile::{reconcile, Source},
    reply::{Kind as ReplyKind, State as ReplyState},
    strategy::Strategy,
    types::{Ica, Icq},
};

fn required_ica_icq_deposit(ica_register_fee: &Coin, icq_deposit_fee: &Coin) -> u128 {
    // Main + Rewards Pot
    const ICAS_REQUIRED: u128 = 2;
    // Main Undelegated Balance + Main Delegations + Rewards Pot Balance
    const ICQS_REQUIRED: u128 = 3;

    let total_ica_register_fee = ica_register_fee.amount.u128() * ICAS_REQUIRED;

    let total_icq_deposit_fee = icq_deposit_fee.amount.u128() * ICQS_REQUIRED;

    total_ica_register_fee + total_icq_deposit_fee
}

fn ibc_denom(channel: &str, remote_denom: &str) -> String {
    let ibc_denom_suffix_bytes =
        hmac_sha256::Hash::hash(format!("transfer/{channel}/{remote_denom}").as_bytes());

    let ibc_denom_suffix_str = hex::encode_upper(ibc_denom_suffix_bytes);

    format!("ibc/{ibc_denom_suffix_str}")
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<NeutronMsg>> {
    ensure!(
        !msg.config.connection_id.is_empty(),
        "connection ID is not empty"
    );
    ensure!(
        !msg.config.transfer_out_channel.is_empty(),
        "transfer out channel is not empty"
    );
    ensure!(
        !msg.config.transfer_in_channel.is_empty(),
        "transfer in channel is not empty"
    );
    ensure!(
        !msg.config.remote_denom.is_empty(),
        "remote denom is not empty"
    );
    ensure!(
        !msg.initial_validator_set.is_empty(),
        "initial validator set is not empty"
    );
    ensure!(
        msg.initial_validator_set.len() == msg.initial_validator_weights.len(),
        "initial validator set length matches validator set weights length"
    );

    let ica_register_fee = deps.querier.interchain_account_register_fee()?;

    let icq_deposit = deps.querier.interchain_query_deposit()?;

    ensure!(
        ica_register_fee.denom == icq_deposit.denom,
        "ica & icq fee denoms match"
    );

    let required_deposit = required_ica_icq_deposit(&ica_register_fee, &icq_deposit);

    let deposit = must_pay(&info, &ica_register_fee.denom)?;

    if required_deposit != deposit.u128() {
        bail!(
            "{required_deposit} {} required to initialise contract",
            ica_register_fee.denom
        );
    }

    let max_msg_count = deps.querier.interchain_tx_max_msg_count()?;

    let store = deps.storage;
    let config = msg.config;

    let weights = msg
        .initial_validator_weights
        .into_iter()
        .map(Weight::checked_from_bps)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow!("invalid initial validator slot weight"))?;

    let weights =
        Weights::new(&weights).ok_or_else(|| anyhow!("invalid initial validator slot weights"))?;

    admin::init(store, &info);

    let ibc_deposit_asset = ibc_denom(&config.transfer_out_channel, &config.remote_denom);

    store.set_connection_id(&config.connection_id);
    store.set_estimated_block_interval_seconds(config.estimated_block_interval_seconds);
    store.set_fee_bps_block_increment(config.fee_bps_block_increment);
    store.set_fee_payment_cooldown_blocks(config.fee_payment_cooldown_blocks);
    store.set_ibc_deposit_asset(&ibc_deposit_asset);
    store.set_icq_update_interval(config.icq_update_interval);
    store.set_interchain_tx_timeout_seconds(config.interchain_tx_timeout_seconds);
    store.set_max_fee_bps(config.max_fee_bps);
    store.set_max_ibc_msg_count(max_msg_count);
    store.set_max_unbonding_entries(config.max_unbonding_entries);
    store.set_minimum_unbond_interval(config.unbonding_period / config.max_unbonding_entries);
    store.set_remote_denom(&config.remote_denom);
    store.set_remote_denom_decimals(config.remote_denom_decimals);
    store.set_transfer_in_channel(&config.transfer_in_channel);
    store.set_transfer_in_timeout_seconds(config.transfer_in_timeout_seconds);
    store.set_transfer_out_channel(&config.transfer_out_channel);
    store.set_transfer_out_timeout_seconds(config.transfer_out_timeout_seconds);
    store.set_unbonding_period(config.unbonding_period);
    store.set_validator_set_size(msg.initial_validator_set.len());

    for (slot_idx, (validator, weight)) in msg
        .initial_validator_set
        .into_iter()
        .zip(weights.iter().copied())
        .enumerate()
    {
        store.set_validator(slot_idx, &validator);
        store.set_validator_weight(slot_idx, weight);
    }

    let init_mint_msg = init_mint_msg(TokenFactory::new(&env));

    Ok(Response::default()
        .add_message(init_mint_msg)
        .add_messages([
            NeutronMsg::RegisterInterchainAccount {
                connection_id: config.connection_id.clone(),
                interchain_account_id: Ica::Main.id().to_owned(),
                register_fee: Some(vec![ica_register_fee.clone()]),
            },
            NeutronMsg::RegisterInterchainAccount {
                connection_id: config.connection_id,
                interchain_account_id: Ica::Rewards.id().to_owned(),
                register_fee: Some(vec![ica_register_fee.clone()]),
            },
        ]))
}

pub fn execute_admin_msg(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response<NeutronMsg>> {
    let repository = AdminRepository::new(deps.storage);

    let cmd = admin::handle_execute_msg(deps.api, &repository, info, msg)?;

    admin::handle_cmd(deps.storage, cmd);

    Ok(Response::default())
}

pub fn execute_vault_msg(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: VaultExecuteMsg,
) -> Result<Response<NeutronMsg>> {
    let strategy = Strategy::new(deps.storage, &env);

    let unbonding_log = UnbondingLog::new(deps.storage);

    let mint = SharesMint::new(deps.storage, &env);

    let (cmds, mut response) =
        vault::handle_execute_msg(&strategy, &unbonding_log, &mint, info, msg)?;

    for cmd in cmds {
        match cmd {
            VaultCmd::Mint(cmd) => {
                let msg = handle_mint_cmd(deps.storage, TokenFactory::new(&env), cmd);

                response.messages.push(SubMsg::new(msg));
            }

            VaultCmd::Strategy(cmd) => {
                if let Some(msg) = strategy::handle_cmd(deps.storage, cmd) {
                    response.messages.push(SubMsg::new(msg));
                }
            }

            VaultCmd::UnbondingLog(cmd) => handle_unbonding_log_cmd(deps.storage, cmd),
        }
    }

    Ok(response)
}

pub fn execute_strategy_msg(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: StrategyExecuteMsg,
) -> Result<Response<NeutronMsg>> {
    match msg {
        StrategyExecuteMsg::Reconcile { fee_recipient } => {
            reconcile(deps, env, Source::Trigger(info, fee_recipient))
        }

        StrategyExecuteMsg::ReceiveUndelegated {} => strategy::handle_receive_unbonded(deps, info),

        StrategyExecuteMsg::RestoreIca { id } => {
            let Some(ica) = Ica::from_id(&id) else {
                bail!("unrecognised ica id: {id}");
            };

            strategy::handle_restore_ica(deps, info, ica)
        }

        StrategyExecuteMsg::RestoreIcq { id } => {
            let Some(icq) = Icq::from_id(&id) else {
                bail!("unrecognised icq id: {id}");
            };

            strategy::handle_restore_icq(deps, info, icq)
        }

        StrategyExecuteMsg::ResetMaxMsgCount {} => {
            if deps.storage.reconcile_state().is_pending() {
                bail!("cannot reset max msg count while reconcile is pending");
            }

            let max_msg_count = deps.querier.interchain_tx_max_msg_count()?;

            deps.storage.set_max_ibc_msg_count(max_msg_count);

            Ok(Response::default())
        }

        StrategyExecuteMsg::UpdateConfig {
            estimated_block_interval_seconds,
            fee_bps_block_increment,
            fee_payment_cooldown_blocks,
            icq_update_interval,
            interchain_tx_timeout_seconds,
            max_fee_bps,
            transfer_in_timeout_seconds,
            transfer_out_timeout_seconds,
        } => {
            let repository = AdminRepository::new(deps.storage);

            admin::get_admin_role(&repository, info)?;

            if let Some(v) = estimated_block_interval_seconds {
                deps.storage.set_estimated_block_interval_seconds(v);
            }

            if let Some(v) = fee_bps_block_increment {
                deps.storage.set_fee_bps_block_increment(v);
            }

            if let Some(v) = fee_payment_cooldown_blocks {
                deps.storage.set_fee_payment_cooldown_blocks(v);
            }

            if let Some(v) = icq_update_interval {
                deps.storage.set_icq_update_interval(v);
            }

            if let Some(v) = interchain_tx_timeout_seconds {
                deps.storage.set_interchain_tx_timeout_seconds(v);
            }

            if let Some(v) = max_fee_bps {
                deps.storage.set_max_fee_bps(v);
            }

            if let Some(v) = transfer_in_timeout_seconds {
                deps.storage.set_transfer_in_timeout_seconds(v);
            }

            if let Some(v) = transfer_out_timeout_seconds {
                deps.storage.set_transfer_out_timeout_seconds(v);
            }

            Ok(Response::default())
        }
    }
}

#[entry_point]
pub fn execute(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<NeutronMsg>> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => execute_admin_msg(deps, env, info, admin_msg),
        ExecuteMsg::Vault(vault_msg) => execute_vault_msg(deps, env, info, vault_msg),
        ExecuteMsg::Strategy(strategy_msg) => execute_strategy_msg(deps, env, info, strategy_msg),
    }
}

pub fn handle_strategy_query(deps: Deps<NeutronQuery>, query: StrategyQueryMsg) -> Result<Binary> {
    let response = match query {
        StrategyQueryMsg::Config {} => to_json_binary(&Config {
            connection_id: deps.storage.connection_id(),
            estimated_block_interval_seconds: deps.storage.estimated_block_interval_seconds(),
            fee_bps_block_increment: deps.storage.fee_bps_block_increment(),
            fee_payment_cooldown_blocks: deps.storage.fee_payment_cooldown_blocks(),
            icq_update_interval: deps.storage.icq_update_interval(),
            interchain_tx_timeout_seconds: deps.storage.interchain_tx_timeout_seconds(),
            max_fee_bps: deps.storage.max_fee_bps(),
            max_unbonding_entries: deps.storage.max_unbonding_entries(),
            remote_denom: deps.storage.remote_denom(),
            remote_denom_decimals: deps.storage.remote_denom_decimals(),
            transfer_in_channel: deps.storage.transfer_in_channel(),
            transfer_in_timeout_seconds: deps.storage.transfer_in_timeout_seconds(),
            transfer_out_channel: deps.storage.transfer_out_channel(),
            transfer_out_timeout_seconds: deps.storage.transfer_out_timeout_seconds(),
            unbonding_period: deps.storage.unbonding_period(),
        })?,

        StrategyQueryMsg::Metadata {} => to_json_binary(&Metadata {
            available_to_claim: deps.storage.available_to_claim().0.into(),
            delegated: deps.storage.delegated().0.into(),
            delegations_icq: deps.storage.delegations_icq(),
            ibc_deposit_asset: deps.storage.ibc_deposit_asset(),
            inflight_delegation: deps.storage.inflight_delegation().0.into(),
            inflight_deposit: deps.storage.inflight_deposit().0.into(),
            inflight_fee_payable: deps.storage.inflight_fee_payable().0.into(),
            inflight_rewards_receivable: deps.storage.inflight_rewards_receivable().0.into(),
            inflight_unbond: deps.storage.inflight_unbond().0.into(),
            last_reconcile_height: deps.storage.last_reconcile_height().map(|height| height.0),
            last_unbond_timestamp: deps.storage.last_unbond_timestamp(),
            last_main_ica_balance_icq_update: deps.storage.last_main_ica_balance_icq_update(),
            last_used_main_ica_balance_icq_update: deps
                .storage
                .last_used_main_ica_balance_icq_update(),
            main_ica_address: deps.storage.main_ica_address(),
            main_ica_balance_icq: deps.storage.main_ica_balance_icq(),
            max_ibc_msg_count: deps.storage.max_ibc_msg_count(),
            minimum_unbond_interval: deps.storage.minimum_unbond_interval(),
            msg_issued_count: deps.storage.msg_issued_count().0,
            msg_success_count: deps.storage.msg_success_count().0,
            pending_deposit: deps.storage.pending_deposit().0.into(),
            pending_unbond: deps.storage.pending_unbond().0.into(),
            rewards_ica_address: deps.storage.rewards_ica_address(),
            rewards_ica_balance_icq: deps.storage.rewards_ica_balance_icq(),
            total_actual_unbonded: deps.storage.total_actual_unbonded().0.into(),
            total_expected_unbonded: deps.storage.total_expected_unbonded().0.into(),
            unbonding_ack_count: deps.storage.unbonding_ack_count(),
            unbonding_issued_count: deps.storage.unbonding_issued_count(),
        })?,

        StrategyQueryMsg::ReconcileState {} => {
            let phase = deps.storage.reconcile_phase();
            let state = deps.storage.reconcile_state();

            let cost = reconcile_cost(deps, phase, state).map(|coin| coin.amount)?;

            to_json_binary(&ReconcileState {
                fee_recipient: deps.storage.fee_recipient(),
                phase: phase.to_string().to_lowercase(),
                state: state.to_string().to_lowercase(),
                trigger_address: deps.storage.reconcile_trigger_address(),
                cost,
            })?
        }

        StrategyQueryMsg::ValidatorSet {} => to_json_binary(&ValidatorSet {
            size: deps.storage.validator_set_size(),
            validators: deps.storage.validators(),
            weights: deps
                .storage
                .validator_weights()
                .iter()
                .map(ToString::to_string)
                .collect(),
        })?,
    };

    Ok(response)
}

#[entry_point]
pub fn query(deps: Deps<NeutronQuery>, env: Env, msg: QueryMsg) -> Result<Binary> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            let repository = AdminRepository::new(deps.storage);

            admin::handle_query_msg(&repository, admin_query)?
        }

        QueryMsg::Vault(vault_query) => vault::handle_query_msg(
            deps.storage,
            &Strategy::new(deps.storage, &env),
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
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response<NeutronMsg>> {
    let ReplyState { kind, ica } = ReplyState::from(reply.id);

    match kind {
        ReplyKind::RegisterDelegationIcq => reply::handle_register_delegations_icq(deps, reply),

        ReplyKind::RegisterBalanceIcq => reply::handle_register_balance_icq(deps, ica, reply),
    }
}

#[entry_point]
pub fn sudo(deps: DepsMut<NeutronQuery>, env: Env, msg: SudoMsg) -> Result<Response<NeutronMsg>> {
    match msg {
        SudoMsg::OpenAck {
            port_id,
            counterparty_version,
            ..
        } => sudo::handle_open_ack(deps, port_id, counterparty_version),

        SudoMsg::Response { .. } => sudo::handle_response(deps, env),

        SudoMsg::Error { .. } => sudo::handle_error(deps, env),

        SudoMsg::Timeout { .. } => sudo::handle_timeout(deps, env),

        SudoMsg::KVQueryResult { query_id } => sudo::handle_kv_query_result(deps, env, query_id),

        SudoMsg::TxQueryResult { .. } => Ok(Response::default()),
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response> {
    Ok(Response::default())
}
