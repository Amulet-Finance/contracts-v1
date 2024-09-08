pub mod msg;

use anyhow::{bail, Error};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Storage,
    WasmMsg,
};

use amulet_cw::{
    admin::{self, get_admin_role, Repository as AdminRepository},
    hub::UserMsg as HubMsg,
    StorageExt as _,
};
use msg::{ConfigResponse, ProxyExecuteMsg, ProxyQueryMsg, WhitelistedResponse};

use self::msg::{AdminExecuteMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

macro_rules! whitelisted_key {
    ($addr:ident) => {
        format!("whitelisted:{}", $addr)
    };
}

trait StorageExt: Storage {
    fn set_hub(&mut self, address: &str) {
        self.set_string("hub", address)
    }

    fn hub(&self) -> String {
        self.string_at("hub")
            .expect("always: set during initialisation")
    }

    fn set_whitelisted(&mut self, address: &str, whitelisted: bool) {
        self.set_bool(whitelisted_key!(address), whitelisted)
    }

    fn whitelisted(&self, address: &str) -> bool {
        self.bool_at(whitelisted_key!(address)).unwrap_or(false)
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    deps.storage.set_hub(&msg.hub_address);

    Ok(Response::default())
}

macro_rules! ensure_whitelisted {
    ($deps:ident, $info:ident) => {
        if !$deps.storage.whitelisted($info.sender.as_str()) {
            bail!("{} is not whitelisted", $info.sender);
        }
    };
}

macro_rules! forward_to_hub {
    ($deps:ident, $info:ident, $msg:expr) => {
        Ok(Response::default().add_message(WasmMsg::Execute {
            contract_addr: $deps.storage.hub(),
            msg: to_json_binary(&$msg)?,
            funds: $info.funds,
        }))
    };
}

pub fn execute_proxy_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ProxyExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ProxyExecuteMsg::SetWhitelisted {
            address,
            whitelisted,
        } => {
            get_admin_role(&AdminRepository::new(deps.storage), &info)?;

            deps.storage.set_whitelisted(&address, whitelisted);

            Ok(Response::default())
        }

        ProxyExecuteMsg::Deposit { vault } => {
            ensure_whitelisted!(deps, info);
            forward_to_hub!(
                deps,
                info,
                HubMsg::DepositOnBehalf {
                    vault,
                    behalf_of: info.sender.into_string(),
                }
            )
        }

        ProxyExecuteMsg::Mint { vault } => {
            ensure_whitelisted!(deps, info);
            forward_to_hub!(
                deps,
                info,
                HubMsg::MintOnBehalf {
                    vault,
                    behalf_of: info.sender.into_string(),
                }
            )
        }

        ProxyExecuteMsg::Advance { vault, amount } => {
            ensure_whitelisted!(deps, info);
            forward_to_hub!(
                deps,
                info,
                HubMsg::AdvanceOnBehalf {
                    vault,
                    amount,
                    behalf_of: info.sender.into_string(),
                }
            )
        }

        ProxyExecuteMsg::Redeem { vault } => {
            ensure_whitelisted!(deps, info);
            forward_to_hub!(
                deps,
                info,
                HubMsg::RedeemOnBehalf {
                    vault,
                    behalf_of: info.sender.into_string(),
                }
            )
        }
    }
}

pub fn execute_admin_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: AdminExecuteMsg,
) -> Result<Response, Error> {
    let cmd = admin::handle_execute_msg(deps.api, &AdminRepository::new(deps.storage), info, msg)?;

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
        ExecuteMsg::Proxy(proxy_msg) => execute_proxy_msg(deps, env, info, proxy_msg),
    }
}

pub fn handle_proxy_query(deps: Deps, msg: ProxyQueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        ProxyQueryMsg::Config {} => to_json_binary(&ConfigResponse {
            hub_address: deps.storage.hub(),
        })?,

        ProxyQueryMsg::Whitelisted { address } => to_json_binary(&WhitelistedResponse {
            whitelisted: deps.storage.whitelisted(&address),
        })?,
    };

    Ok(binary)
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(admin_query) => {
            admin::handle_query_msg(&AdminRepository::new(deps.storage), admin_query)?
        }

        QueryMsg::Proxy(proxy_query) => handle_proxy_query(deps, proxy_query)?,
    };

    Ok(binary)
}
