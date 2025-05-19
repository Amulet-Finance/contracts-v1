pub mod msg;

use anyhow::Error;
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, Storage};

use amulet_cw::{
    admin::{self, Repository as AdminRespository},
    mint::{self, Repository as MintRepository},
    MigrateMsg, StorageExt as _,
};
use amulet_token_factory::Flavour as TokenFactoryFlavour;

use self::msg::{AdminExecuteMsg, ExecuteMsg, InstantiateMsg, MintExecuteMsg, QueryMsg};

const TOKEN_FACTORY_FLAVOUR_KEY: &str = "amulet_mint::token_factory_flavour";

fn token_factory_flavour(storage: &dyn Storage) -> TokenFactoryFlavour {
    storage
        .u8_at(TOKEN_FACTORY_FLAVOUR_KEY)
        .expect("set during initialisation")
        .into()
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    deps.storage
        .set_u8(TOKEN_FACTORY_FLAVOUR_KEY, msg.token_factory_flavour.into());

    Ok(Response::default())
}

pub fn execute_mint_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: MintExecuteMsg,
) -> Result<Response, Error> {
    let cmd = mint::handle_execute_msg(
        deps.api,
        &AdminRespository::new(deps.storage),
        &MintRepository::new(deps.storage),
        info,
        msg,
    )?;

    let token_factory = token_factory_flavour(deps.storage).into_factory(&env);

    let sub_msgs = mint::handle_cmd(deps.storage, token_factory, cmd);

    Ok(Response::default().add_submessages(sub_msgs))
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
        ExecuteMsg::Mint(mint_msg) => execute_mint_msg(deps, env, info, mint_msg),
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            admin::handle_query_msg(&AdminRespository::new(deps.storage), admin_query)?
        }

        QueryMsg::Mint(mint_query) => mint::handle_query_msg(deps.storage, mint_query)?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}

#[cfg(test)]
mod test;
