use anyhow::Result;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{from_json, DepsMut, Env, Response};
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};

use crate::{
    icq,
    reconcile::{reconcile, Source, Status},
    state::StorageExt,
    types::Ica,
};

#[must_use]
fn ica_from_port_id(port_id: &str) -> Ica {
    port_id
        .split('.')
        .last()
        .and_then(Ica::from_id)
        .expect("always: ica present in port id")
}

pub fn handle_main_ica_registered(
    deps: DepsMut<NeutronQuery>,
    address: &str,
) -> Response<NeutronMsg> {
    deps.storage.set_main_ica_address(address);

    let mut response = Response::default();

    if deps.storage.main_ica_balance_icq().is_none() {
        let msg = icq::ica_balance_registration_msg(deps.storage, Ica::Main);

        response.messages.push(msg);
    }

    if deps.storage.delegations_icq().is_none() {
        let msg = icq::main_ica_delegations_registration_msg(deps.storage);

        response.messages.push(msg);
    }

    response
}

pub fn handle_rewards_ica_registered(
    deps: DepsMut<NeutronQuery>,
    address: &str,
) -> Response<NeutronMsg> {
    deps.storage.set_rewards_ica_address(address);

    if deps.storage.rewards_ica_balance_icq().is_some() {
        return Response::default();
    }

    let msg = icq::ica_balance_registration_msg(deps.storage, Ica::Rewards);

    Response::default().add_submessage(msg)
}

#[cw_serde]
struct OpenAckVersion {
    pub version: String,
    pub controller_connection_id: String,
    pub host_connection_id: String,
    pub address: String,
    pub encoding: String,
    pub tx_type: String,
}

pub fn handle_open_ack(
    deps: DepsMut<NeutronQuery>,
    port_id: String,
    counterparty_version: String,
) -> Result<Response<NeutronMsg>> {
    let parsed_version: OpenAckVersion =
        from_json(counterparty_version).expect("valid counterparty_version");

    let ica = ica_from_port_id(&port_id);

    let response = match ica {
        Ica::Main => handle_main_ica_registered(deps, &parsed_version.address),

        Ica::Rewards => handle_rewards_ica_registered(deps, &parsed_version.address),
    };

    Ok(response)
}

pub fn handle_response(deps: DepsMut<NeutronQuery>, env: Env) -> Result<Response<NeutronMsg>> {
    reconcile(deps, env, Source::Continuation(Status::Success))
}

pub fn handle_error(deps: DepsMut<NeutronQuery>, env: Env) -> Result<Response<NeutronMsg>> {
    reconcile(deps, env, Source::Continuation(Status::Failure))
}

pub fn handle_timeout(deps: DepsMut<NeutronQuery>, env: Env) -> Result<Response<NeutronMsg>> {
    reconcile(deps, env, Source::Continuation(Status::Failure))
}

pub fn handle_kv_query_result(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    query_id: u64,
) -> Result<Response<NeutronMsg>> {
    let Some(main_balance_query_id) = deps.storage.main_ica_balance_icq() else {
        return Ok(Response::default());
    };

    if query_id != main_balance_query_id {
        return Ok(Response::default());
    }

    deps.storage
        .set_last_main_ica_balance_icq_update(env.block.time.seconds());

    Ok(Response::default())
}
