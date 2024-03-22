pub mod msg;

use anyhow::Error;
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};

use amulet_cw::{
    admin::{self, Repository as AdminRespository},
    hub::{self, AdvanceFeeOracle, BalanceSheet, Ctx, SyntheticMint, Vaults},
    MigrateMsg,
};

use self::msg::{AdminExecuteMsg, ExecuteMsg, HubExecuteMsg, InstantiateMsg, QueryMsg};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    hub::init_mint(deps.api, deps.storage, &msg.synthetic_mint)?;

    Ok(Response::default())
}

pub fn execute_hub_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: HubExecuteMsg,
) -> Result<Response, Error> {
    let vaults = &Vaults::new(deps.storage, deps.querier);

    let admin_repository = &AdminRespository::new(deps.storage);

    let mint = &SyntheticMint::new(deps.storage, deps.querier);

    let balance_sheet = &BalanceSheet::new(deps.storage);

    let advance_fee_oracle = &AdvanceFeeOracle::new(deps.querier);

    let ctx = Ctx {
        api: deps.api,
        vaults,
        admin_repository,
        mint,
        balance_sheet,
        advance_fee_oracle,
    };

    let (cmds, mut response) = hub::handle_execute_msg(ctx, info, msg)?;

    for cmd in cmds {
        hub::handle_hub_cmd(deps.storage, &env, &mut response, cmd)?;
    }

    Ok(response)
}

pub fn execute_admin_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response, Error> {
    let admin_repository = &AdminRespository::new(deps.storage);

    let cmd = admin::handle_execute_msg(deps.api, admin_repository, info, msg)?;

    admin::handle_cmd(deps.storage, cmd);

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => execute_admin_msg(deps, env, info, admin_msg),
        ExecuteMsg::Hub(hub_msg) => execute_hub_msg(deps, env, info, hub_msg),
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            admin::handle_query_msg(&AdminRespository::new(deps.storage), admin_query)?
        }

        QueryMsg::Hub(hub_query) => hub::handle_query_msg(
            deps.storage,
            &Vaults::new(deps.storage, deps.querier),
            &BalanceSheet::new(deps.storage),
            &AdvanceFeeOracle::new(deps.querier),
            hub_query,
        )?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}
