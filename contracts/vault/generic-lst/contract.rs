pub mod msg;
pub mod state;
pub mod strategy;

use anyhow::Error;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    Storage, SubMsg,
};
use neutron_sdk::bindings::msg::NeutronMsg;

use amulet_core::vault::Cmd as VaultCmd;
use amulet_cw::{
    admin::{self, ExecuteMsg as AdminExecuteMsg, Repository as AdminRepository},
    vault::{self, ExecuteMsg as VaultExecuteMsg, UnbondingLog},
    MigrateMsg,
};
use amulet_ntrn::vault::{mint, Mint};

use self::msg::{
    ExecuteMsg, InstantiateMsg, MetadataResponse, QueryMsg, StrategyExecuteMsg, StrategyQueryMsg,
};
use self::state::StorageExt as _;
use self::strategy::{lst_redeption_rate, Strategy};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<NeutronMsg>, Error> {
    admin::init(deps.storage, &info);

    deps.api.addr_validate(&msg.lst_redemption_rate_oracle)?;

    deps.storage
        .set_lst_redemption_rate_oracle(&msg.lst_redemption_rate_oracle);

    deps.storage.set_lst_denom(&msg.lst_denom);

    deps.storage.set_lst_decimals(msg.lst_decimals);

    deps.storage
        .set_underlying_decimals(msg.underlying_decimals);

    Ok(Response::default().add_message(mint::init_msg()))
}

pub fn execute_admin_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
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
) -> Result<Response<NeutronMsg>, Error> {
    let redemption_rate = lst_redeption_rate(deps.storage, deps.querier)?;

    let strategy = Strategy::new(deps.storage, &env, redemption_rate);

    let unbonding_log = UnbondingLog::new(deps.storage);

    let mint = Mint::new(deps.storage, &env);

    let (cmds, mut response) =
        vault::handle_execute_msg(&strategy, &unbonding_log, &mint, info, msg)?;

    for cmd in cmds {
        match cmd {
            VaultCmd::Mint(cmd) => {
                let msg = mint::handle_cmd(deps.storage, &env, cmd);
                response.messages.push(SubMsg::new(msg));
            }

            VaultCmd::Strategy(cmd) => {
                if let Some(msg) = strategy::handle_cmd(deps.storage, redemption_rate, cmd) {
                    response.messages.push(SubMsg::new(msg));
                }
            }

            VaultCmd::UnbondingLog(cmd) => {
                amulet_cw::vault::unbonding_log::handle_cmd(deps.storage, cmd)
            }
        }
    }

    Ok(response)
}

pub fn execute_strategy_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: StrategyExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    match msg {
        StrategyExecuteMsg::SetRedemptionRateOracle { oracle } => {
            let repository = AdminRepository::new(deps.storage);

            let _ = admin::get_admin_role(&repository, info)?;

            deps.storage.set_lst_redemption_rate_oracle(&oracle);
        }
    }

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => execute_admin_msg(deps, env, info, admin_msg),
        ExecuteMsg::Vault(vault_msg) => execute_vault_msg(deps, env, info, vault_msg),
        ExecuteMsg::Strategy(strategy_msg) => execute_strategy_msg(deps, env, info, strategy_msg),
    }
}

pub fn handle_strategy_query(
    storage: &dyn Storage,
    query: StrategyQueryMsg,
) -> Result<Binary, StdError> {
    match query {
        StrategyQueryMsg::Metadata {} => to_json_binary(&MetadataResponse {
            lst_redemption_rate_oracle: storage.lst_redemption_rate_oracle(),
            lst_denom: storage.lst_denom(),
            lst_decimals: storage.lst_decimals(),
            underlying_decimals: storage.underlying_decimals(),
            active_lst_balance: storage.active_lst_balance().into(),
            claimable_lst_balance: storage.claimable_lst_balance().into(),
        }),
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            let repository = AdminRepository::new(deps.storage);

            admin::handle_query_msg(&repository, admin_query)?
        }

        QueryMsg::Vault(vault_query) => {
            let redemption_rate = lst_redeption_rate(deps.storage, deps.querier)?;

            vault::handle_query_msg(
                deps.storage,
                &Strategy::new(deps.storage, &env, redemption_rate),
                &UnbondingLog::new(deps.storage),
                &Mint::new(deps.storage, &env),
                &env,
                vault_query,
            )?
        }

        QueryMsg::Strategy(strategy_query) => handle_strategy_query(deps.storage, strategy_query)?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}
