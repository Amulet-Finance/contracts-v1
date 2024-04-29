use cosmwasm_std::{Storage, SubMsg};
use neutron_sdk::{
    bindings::msg::NeutronMsg,
    interchain_queries::v047::register_queries::{
        new_register_balance_query_msg, new_register_delegator_delegations_query_msg,
    },
};

use crate::{
    reply::{Kind as ReplyKind, State as ReplyState},
    state::StorageExt as _,
    types::Ica,
};

pub fn ica_balance_registration_msg(storage: &dyn Storage, ica: Ica) -> SubMsg<NeutronMsg> {
    let connection_id = storage.connection_id();

    let icq_update_period = storage.icq_update_interval();

    let balance_icq_denom = storage.remote_denom();

    let ica_address = match ica {
        Ica::Main => storage.main_ica_address(),
        Ica::Rewards => storage.rewards_ica_address(),
    };

    let Some(ica_address) = ica_address else {
        panic!("{} ICA not registered", ica.id());
    };

    let balance_icq_register_msg = new_register_balance_query_msg(
        connection_id,
        ica_address,
        balance_icq_denom,
        icq_update_period,
    )
    .expect("infallible message construction");

    SubMsg::reply_always(
        balance_icq_register_msg,
        ReplyState {
            kind: ReplyKind::RegisterBalanceIcq,
            ica,
        }
        .into(),
    )
}

fn delegations_registration_msg(
    storage: &dyn Storage,
    validator_set: Vec<String>,
    kind: ReplyKind,
) -> SubMsg<NeutronMsg> {
    let connection_id = storage.connection_id();

    let icq_update_period = storage.icq_update_interval();

    let Some(ica_address) = storage.main_ica_address() else {
        panic!("{} ICA not registered", Ica::Main.id());
    };

    let msg = new_register_delegator_delegations_query_msg(
        connection_id,
        ica_address,
        validator_set,
        icq_update_period,
    )
    .expect("infallible message construction");

    SubMsg::reply_always(
        msg,
        ReplyState {
            kind,
            ica: Ica::Main,
        }
        .into(),
    )
}

pub fn main_ica_current_delegations_registration_msg(
    storage: &dyn Storage,
    validator_set: Vec<String>,
) -> SubMsg<NeutronMsg> {
    delegations_registration_msg(
        storage,
        validator_set,
        ReplyKind::RegisterCurrentSetDelegationsIcq,
    )
}

pub fn main_ica_next_delegations_registration_msg(
    storage: &dyn Storage,
    validator_set: Vec<String>,
) -> SubMsg<NeutronMsg> {
    delegations_registration_msg(
        storage,
        validator_set,
        ReplyKind::RegisterNextSetDelegationsIcq,
    )
}
