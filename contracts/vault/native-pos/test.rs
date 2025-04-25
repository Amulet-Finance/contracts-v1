use amulet_core::vault::{DepositValue, Strategy as _, UnbondReadyStatus};
use amulet_token_factory::Flavour;
use cosmwasm_std::{
    testing::{mock_dependencies, mock_env},
    Addr, Decimal, MessageInfo, QuerierWrapper, Validator,
};

use test_utils::{check, prelude::expect};

use crate::{instantiate, state::StorageExt, strategy::Strategy, InstantiateMsg};

macro_rules! info {
    ($sender:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: vec![],
        }
    };
    ($sender:literal, $amount:literal, $asset:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: coins($amount, $asset),
        }
    };
}

macro_rules! deps {
    () => {{
        let mut deps = mock_dependencies();

        let all_validators: Vec<_> = (0..10)
            .into_iter()
            .map(|i| Validator {
                address: format!("val{i}"),
                commission: Decimal::percent(5),
                max_commission: Decimal::percent(10),
                max_change_rate: Decimal::percent(1),
            })
            .collect();

        deps.querier.update_staking("stake", &all_validators, &[]);
        deps
    }};
}

#[test]
fn instantiate_non_unique_validator_set_fails() {
    let mut deps = deps!();

    let err = instantiate(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        InstantiateMsg {
            rewards_sink_code_id: 0,
            rewards_sink_code_hash: "deadbeef".to_owned(),
            token_factory_flavour: Flavour::Osmosis,
            bond_denom: "stake".to_owned(),
            bond_denom_decimals: 6,
            max_unbonding_entries: 7,
            unbonding_period: 60 * 60 * 24 * 21,
            initial_validator_set: vec![
                "val1".to_owned(),
                "val2".to_owned(),
                "val2".to_owned(),
                "val3".to_owned(),
            ],
        },
    )
    .unwrap_err();

    check(
        err.to_string(),
        expect![[r#""validator val2 occurs more than once in the validator set""#]],
    )
}

#[test]
fn strategy_unbond_start_hint() {
    let mut deps = deps!();

    let mut env = mock_env();

    let max_unbonding_entries = 7;
    let unbonding_period = 60 * 60 * 24 * 21;

    instantiate(
        deps.as_mut(),
        env.clone(),
        info!("creator"),
        InstantiateMsg {
            rewards_sink_code_id: 0,
            rewards_sink_code_hash: "deadbeef".to_owned(),
            token_factory_flavour: Flavour::Osmosis,
            bond_denom: "stake".to_owned(),
            bond_denom_decimals: 6,
            max_unbonding_entries: 7,
            unbonding_period: 60 * 60 * 24 * 21,
            initial_validator_set: vec![
                "val1".to_owned(),
                "val2".to_owned(),
                "val3".to_owned(),
                "val4".to_owned(),
            ],
        },
    )
    .unwrap();

    deps.storage
        .set_last_unbond_timestamp(env.block.time.seconds());

    let minimum_unbond_interval = unbonding_period / max_unbonding_entries;

    env.block.time = env.block.time.plus_seconds(minimum_unbond_interval / 2);

    let unbond_status = Strategy::new(&deps.storage, QuerierWrapper::new(&deps.querier), &env)
        .unbond(DepositValue(1_000_000_000));

    assert_eq!(
        unbond_status,
        UnbondReadyStatus::Later(Some(
            deps.storage.last_unbond_timestamp() + minimum_unbond_interval
        ))
    )
}
