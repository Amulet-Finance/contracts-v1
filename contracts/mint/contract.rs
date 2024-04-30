pub mod msg;

use anyhow::Error;
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use neutron_sdk::bindings::msg::NeutronMsg;

use amulet_cw::{
    admin::{self, Repository as AdminRespository},
    mint::{self, Repository as MintRepository},
    MigrateMsg,
};
use amulet_ntrn::token_factory::TokenFactory;

use self::msg::{AdminExecuteMsg, ExecuteMsg, InstantiateMsg, MintExecuteMsg, QueryMsg};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    Ok(Response::default())
}

pub fn execute_mint_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: MintExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    let cmd = mint::handle_execute_msg(
        deps.api,
        &AdminRespository::new(deps.storage),
        &MintRepository::new(deps.storage),
        info,
        msg,
    )?;

    let sub_msgs = mint::handle_cmd(deps.storage, TokenFactory::new(&env), cmd);

    Ok(Response::default().add_submessages(sub_msgs))
}

pub fn execute_admin_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
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
) -> Result<Response<NeutronMsg>, Error> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => execute_admin_msg(deps, env, info, admin_msg),
        ExecuteMsg::Mint(mint_msg) => execute_mint_msg(deps, env, info, mint_msg),
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            admin::handle_query_msg(&AdminRespository::new(deps.storage), admin_query)?
        }

        QueryMsg::Mint(mint_query) => {
            mint::handle_query_msg(deps.storage, deps.querier, mint_query)?
        }
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}
