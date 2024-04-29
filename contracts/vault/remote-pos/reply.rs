use anyhow::Result;
use cosmwasm_schema::{cw_serde, serde::de::DeserializeOwned};
use cosmwasm_std::{from_json, DepsMut, Reply, Response};
use neutron_sdk::bindings::msg::NeutronMsg;

use crate::{state::StorageExt, types::Ica};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Kind {
    RegisterCurrentSetDelegationsIcq = 0,
    RegisterBalanceIcq = 1,
    RegisterNextSetDelegationsIcq = 2,
}

impl From<u8> for Kind {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::RegisterCurrentSetDelegationsIcq,
            1 => Self::RegisterBalanceIcq,
            2 => Self::RegisterNextSetDelegationsIcq,
            _ => panic!("unexpected kind: {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub kind: Kind,
    pub ica: Ica,
}

impl From<State> for u64 {
    fn from(value: State) -> Self {
        u64::from_be_bytes([value.kind as u8, value.ica as u8, 0, 0, 0, 0, 0, 0])
    }
}

impl From<u64> for State {
    fn from(value: u64) -> Self {
        let [kind_u8, ica_u8, ..] = value.to_be_bytes();

        State {
            kind: kind_u8.into(),
            ica: ica_u8.into(),
        }
    }
}

fn extract_reply_data<T: DeserializeOwned>(reply: Reply) -> T {
    let res = reply
        .result
        .into_result()
        .expect("always: submessage issued reply-on-success");

    let data = res.data.expect("always: reply contains data");

    from_json(data).expect("always: infallible deserialisation of reply data")
}

fn parse_icq_registration_reply(reply: Reply) -> u64 {
    #[cw_serde]
    struct MsgRegisterInterchainQueryResponse {
        id: u64,
    }

    let msg: MsgRegisterInterchainQueryResponse = extract_reply_data(reply);

    msg.id
}

pub fn handle_register_current_set_delegations_icq(
    deps: DepsMut,
    reply: Reply,
) -> Result<Response<NeutronMsg>> {
    let delegations_icq_id = parse_icq_registration_reply(reply);

    deps.storage.set_delegations_icq(delegations_icq_id);

    Ok(Response::default())
}

pub fn handle_register_next_set_delegations_icq(
    deps: DepsMut,
    reply: Reply,
) -> Result<Response<NeutronMsg>> {
    let delegations_icq_id = parse_icq_registration_reply(reply);

    deps.storage.set_delegations_icq(delegations_icq_id);

    Ok(Response::default())
}

pub fn handle_register_balance_icq(
    deps: DepsMut,
    ica: Ica,
    reply: Reply,
) -> Result<Response<NeutronMsg>> {
    let icq_id = parse_icq_registration_reply(reply);

    match ica {
        Ica::Main => deps.storage.set_main_ica_balance_icq(icq_id),
        Ica::Rewards => deps.storage.set_rewards_ica_balance_icq(icq_id),
    }

    Ok(Response::default())
}
