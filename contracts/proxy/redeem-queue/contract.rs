pub mod msg;
pub mod queue;
pub mod state;

use anyhow::Error;
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};

use amulet_core::admin::AdminRole;
use amulet_cw::{
    admin::{self, Repository as AdminRepository},
    hub::{QueryMsg as HubQueryMsg, VaultMetadata},
};

use self::{
    msg::{ExecuteMsg, InstantiateMsg, ProxyAdminMsg, ProxyQueryMsg, ProxyUserMsg, QueryMsg},
    state::StorageExt as _,
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, Error> {
    admin::init(deps.storage, &info);

    deps.api.addr_validate(&msg.hub_address)?;

    deps.storage.set_hub(&msg.hub_address);

    Ok(Response::default())
}

fn synthetic_for_vault(deps: Deps, hub: &str, vault: &str) -> Result<String, Error> {
    let vault_metadata: VaultMetadata = deps.querier.query_wasm_smart(
        hub,
        &HubQueryMsg::VaultMetadata {
            vault: vault.to_owned(),
        },
    )?;

    Ok(vault_metadata.synthetic)
}

fn vault_reserve_balance(deps: Deps, hub: &str, vault: &str) -> Result<u128, Error> {
    let vault_metadata: VaultMetadata = deps.querier.query_wasm_smart(
        hub,
        &HubQueryMsg::VaultMetadata {
            vault: vault.to_owned(),
        },
    )?;

    Ok(vault_metadata.reserve_balance.u128())
}

pub fn handle_process_head(_deps: DepsMut, _vault: String) -> Result<Response, Error> {
    todo!()
}

pub fn handle_redeem(
    _deps: DepsMut,
    _info: MessageInfo,
    _vault: String,
) -> Result<Response, Error> {
    todo!()
}

pub fn handle_cancel_entry(
    _deps: DepsMut,
    _info: MessageInfo,
    _vault: String,
    _index: u64,
) -> Result<Response, Error> {
    todo!()
}

pub fn handle_cancel_all(
    _deps: DepsMut,
    _info: MessageInfo,
    _vault: String,
) -> Result<Response, Error> {
    todo!()
}

pub fn handle_force_cancel_entry(
    _: AdminRole,
    _deps: DepsMut,
    _vault: String,
    _index: u64,
) -> Result<Response, Error> {
    todo!()
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, Error> {
    match msg {
        ExecuteMsg::Admin(msg) => {
            let cmd = admin::handle_execute_msg(
                deps.api,
                &AdminRepository::new(deps.storage),
                info,
                msg,
            )?;

            admin::handle_cmd(deps.storage, cmd);

            Ok(Response::default())
        }

        ExecuteMsg::ProxyUser(ProxyUserMsg::ProcessHead { vault }) => {
            handle_cancel_all(deps, info, vault)
        }

        ExecuteMsg::ProxyUser(ProxyUserMsg::Redeem { vault }) => handle_redeem(deps, info, vault),

        ExecuteMsg::ProxyUser(ProxyUserMsg::CancelEntry { vault, index }) => {
            handle_cancel_entry(deps, info, vault, index)
        }

        ExecuteMsg::ProxyUser(ProxyUserMsg::CancelAll { vault }) => {
            handle_cancel_all(deps, info, vault)
        }

        ExecuteMsg::ProxyAdmin(admin_msg) => {
            let admin_role = admin::get_admin_role(&AdminRepository::new(deps.storage), &info)?;

            match admin_msg {
                ProxyAdminMsg::ForceCancelEntry { vault, index } => {
                    handle_force_cancel_entry(admin_role, deps, vault, index)
                }
            }
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Admin(query) => {
            admin::handle_query_msg(&AdminRepository::new(deps.storage), query)?
        }

        QueryMsg::Proxy(ProxyQueryMsg::Config {}) => todo!(),

        QueryMsg::Proxy(ProxyQueryMsg::AllQueueEntries { .. }) => todo!(),

        QueryMsg::Proxy(ProxyQueryMsg::OwnerQueueEntries { .. }) => todo!(),

        QueryMsg::Proxy(ProxyQueryMsg::QueueEntry { .. }) => todo!(),
    };

    Ok(binary)
}
