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

    SubMsg::reply_on_success(
        balance_icq_register_msg,
        ReplyState {
            kind: ReplyKind::RegisterBalanceIcq,
            ica,
            index: 0,
        }
        .into(),
    )
}

fn delegations_registration_msg(
    storage: &dyn Storage,
    validators: &[String],
    kind: ReplyKind,
    index: u8,
) -> SubMsg<NeutronMsg> {
    let connection_id = storage.connection_id();

    let icq_update_period = storage.icq_update_interval();

    let Some(ica_address) = storage.main_ica_address() else {
        panic!("{} ICA not registered", Ica::Main.id());
    };

    let msg = new_register_delegator_delegations_query_msg(
        connection_id,
        ica_address,
        validators.to_owned(),
        icq_update_period,
    )
    .expect("infallible message construction");

    SubMsg::reply_on_success(
        msg,
        ReplyState {
            kind,
            ica: Ica::Main,
            index,
        }
        .into(),
    )
}

// // Max keys:
// // https://github.com/neutron-org/neutron/blob/v4.2.3/x/interchainqueries/types/tx.go#L15
// // N keys used per delegations ICQ:
// // https://github.com/neutron-org/neutron-sdk/blob/v0.9.0/packages/neutron-sdk/src/interchain_queries/v047/register_queries.rs#L33
// const MAX_VALIDATORS_PER_ICQ: usize = 15;

// pub fn delegations_icq_count(validator_set_size: usize) -> u8 {
//     validator_set_size
//         .div_ceil(MAX_VALIDATORS_PER_ICQ)
//         .try_into()
//         .expect("validator set size never larger than 255 * MAX_VALIDATORS_PER_ICQ")
// }

fn delegations_registration_msgs(
    storage: &dyn Storage,
    validator_set: Vec<String>,
    kind: ReplyKind,
) -> Vec<SubMsg<NeutronMsg>> {
    let icq_count = storage.delegations_icq_count();

    let max_validators_per_delegations_icq = storage.max_validators_per_delegations_icq().into();

    (0..icq_count)
        .zip(validator_set.chunks(max_validators_per_delegations_icq))
        .map(|(index, validators)| delegations_registration_msg(storage, validators, kind, index))
        .collect()
}

pub fn main_ica_current_delegations_registration_msgs(
    storage: &dyn Storage,
    validator_set: Vec<String>,
) -> Vec<SubMsg<NeutronMsg>> {
    delegations_registration_msgs(
        storage,
        validator_set,
        ReplyKind::RegisterCurrentSetDelegationsIcq,
    )
}

pub fn main_ica_next_delegations_registration_msgs(
    storage: &dyn Storage,
    validator_set: Vec<String>,
) -> Vec<SubMsg<NeutronMsg>> {
    delegations_registration_msgs(
        storage,
        validator_set,
        ReplyKind::RegisterNextSetDelegationsIcq,
    )
}
