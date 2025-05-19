use anyhow::{anyhow, Result};
use cosmwasm_std::coins;
use integration_tests::{
    execute_contract, init_accounts, instantiate_contract, query_bank_balance, query_contract,
    query_staking_delegations, query_staking_unbondings, store_contract_artifact,
};
use osmosis_test_tube::{osmosis_std, Account, OsmosisTestApp, Runner};

const TOTAL_VALIDATOR_COUNT: usize = 1;
const INITIAL_VAULT_SET_SIZE: usize = 1;

#[test]
fn works() -> Result<()> {
    let app = OsmosisTestApp::new();

    let [admin, alice] = init_accounts(&app, &coins(1_000_000_000_000_000, "uosmo"))?;

    let (hub_code_id, _) = store_contract_artifact(&app, &admin, "amulet-hub")?;

    let (mint_code_id, _) = store_contract_artifact(&app, &admin, "amulet-mint")?;

    let (native_pos_id, _) = store_contract_artifact(&app, &admin, "amulet-native-pos")?;

    let mint_address = instantiate_contract(
        &app,
        &admin,
        mint_code_id,
        &amulet_mint::msg::InstantiateMsg {
            token_factory_flavour: amulet_token_factory::Flavour::Osmosis,
        },
        &[],
    )?;

    let hub_address = instantiate_contract(
        &app,
        &admin,
        hub_code_id,
        &amulet_hub::msg::InstantiateMsg {
            synthetic_mint: mint_address.clone(),
        },
        &[],
    )?;

    let (rewards_sink_code_id, rewards_sink_code_hash) =
        store_contract_artifact(&app, &admin, "rewards-sink")?;

    let all_validators: osmosis_std::types::cosmos::staking::v1beta1::QueryValidatorsResponse = app
        .query(
            "/cosmos.staking.v1beta1.Query/Validators",
            &osmosis_std::types::cosmos::staking::v1beta1::QueryValidatorsRequest {
                status: "BOND_STATUS_BONDED".to_owned(),
                pagination: None,
            },
        )?;

    assert_eq!(all_validators.validators.len(), TOTAL_VALIDATOR_COUNT);

    let staking_params: osmosis_std::types::cosmos::staking::v1beta1::QueryParamsResponse = app
        .query(
            "/cosmos.staking.v1beta1.Query/Params",
            &osmosis_std::types::cosmos::staking::v1beta1::QueryParamsRequest {},
        )?;

    let staking_params = staking_params
        .params
        .ok_or_else(|| anyhow!("no params returned for staking module"))?;

    let max_unbonding_entries: u64 = staking_params.max_entries.into();

    let unbonding_period: u64 = staking_params
        .unbonding_time
        .ok_or_else(|| anyhow!("no unbonding time staking param"))?
        .seconds
        .try_into()?;

    let native_pos_address = instantiate_contract(
        &app,
        &admin,
        native_pos_id,
        &amulet_native_pos::msg::InstantiateMsg {
            rewards_sink_code_id,
            rewards_sink_code_hash,
            token_factory_flavour: amulet_token_factory::Flavour::Osmosis,
            bond_denom: staking_params.bond_denom.clone(),
            bond_denom_decimals: 6,
            max_unbonding_entries,
            unbonding_period,
            initial_validator_set: all_validators.validators[..INITIAL_VAULT_SET_SIZE]
                .iter()
                .map(|v| v.operator_address.clone())
                .collect(),
        },
        &[],
    )?;

    // configure mint
    execute_contract(
        &app,
        &admin,
        &mint_address,
        &amulet_cw::mint::ExecuteMsg::CreateSynthetic {
            ticker: "amOSMO".to_owned(),
            decimals: 6,
        },
        &[],
    )?;

    execute_contract(
        &app,
        &admin,
        &mint_address,
        &amulet_cw::mint::ExecuteMsg::SetWhitelisted {
            minter: hub_address.clone(),
            whitelisted: true,
        },
        &[],
    )?;

    // configure native-pos vault in hub
    execute_contract(
        &app,
        &admin,
        &hub_address,
        &amulet_cw::hub::AdminMsg::RegisterVault {
            vault: native_pos_address.clone(),
            synthetic: format!("factory/{mint_address}/amosmo"),
        },
        &[],
    )?;

    execute_contract(
        &app,
        &admin,
        &hub_address,
        &amulet_cw::hub::AdminMsg::SetDepositsEnabled {
            vault: native_pos_address.clone(),
            enabled: true,
        },
        &[],
    )?;

    execute_contract(
        &app,
        &admin,
        &hub_address,
        &amulet_cw::hub::AdminMsg::SetAdvanceEnabled {
            vault: native_pos_address.clone(),
            enabled: true,
        },
        &[],
    )?;

    // alice makes first deposit
    let alice_deposit_amount = 100_000_000_000_000;

    execute_contract(
        &app,
        &alice,
        &hub_address,
        &amulet_cw::hub::UserMsg::Deposit {
            vault: native_pos_address.clone(),
        },
        &coins(alice_deposit_amount, "uosmo"),
    )?;

    let alice_position: amulet_cw::hub::PositionResponse = query_contract(
        &app,
        &hub_address,
        &amulet_cw::hub::QueryMsg::Position {
            account: alice.address(),
            vault: native_pos_address.clone(),
        },
    )?;

    assert_eq!(alice_position.collateral.u128(), alice_deposit_amount);

    let all_delegations = query_staking_delegations(&app, &native_pos_address)?;

    assert_eq!(all_delegations.len(), INITIAL_VAULT_SET_SIZE);
    assert!(all_delegations
        .iter()
        .all(|(_, amount)| *amount >= alice_deposit_amount / INITIAL_VAULT_SET_SIZE as u128));
    assert_eq!(
        all_delegations
            .iter()
            .map(|(_, amount)| amount)
            .sum::<u128>(),
        alice_deposit_amount
    );

    // alice makes first withdrawal
    let alice_withdraw_amount = alice_deposit_amount / (max_unbonding_entries * 2) as u128;
    execute_contract(
        &app,
        &alice,
        &hub_address,
        &amulet_cw::hub::UserMsg::Withdraw {
            vault: native_pos_address.clone(),
            amount: alice_withdraw_amount.into(),
        },
        &[],
    )?;

    let pending_batch: amulet_cw::vault::PendingUnbondingResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::PendingUnbonding { address: None },
    )?;

    assert_eq!(pending_batch.amount.u128(), alice_withdraw_amount);

    app.increase_time((unbonding_period / max_unbonding_entries) + 1);

    execute_contract(
        &app,
        &alice,
        &native_pos_address,
        &amulet_cw::vault::ExecuteMsg::StartUnbond {},
        &[],
    )?;

    let unbonding_batches: amulet_cw::vault::ActiveUnbondingsResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::ActiveUnbondings {
            address: None,
            limit: None,
        },
    )?;

    assert_eq!(unbonding_batches.unbondings.len(), 1);

    assert_eq!(
        unbonding_batches.unbondings[0].amount.u128(),
        alice_withdraw_amount
    );
    assert_eq!(
        unbonding_batches.unbondings[0].end,
        app.get_block_timestamp()
            .plus_seconds(unbonding_period)
            .seconds()
    );

    let unbondings = query_staking_unbondings(&app, &native_pos_address)?;

    assert_eq!(unbondings.len(), INITIAL_VAULT_SET_SIZE);

    assert!(unbondings.iter().all(|res| res.entries.len() == 1
        && res.entries[0].amount >= alice_withdraw_amount / INITIAL_VAULT_SET_SIZE as u128));

    // fill up unbondings to max entries
    for i in 1..max_unbonding_entries as usize {
        app.increase_time((unbonding_period / max_unbonding_entries) + 1);

        execute_contract(
            &app,
            &alice,
            &hub_address,
            &amulet_cw::hub::UserMsg::Withdraw {
                vault: native_pos_address.clone(),
                amount: alice_withdraw_amount.into(),
            },
            &[],
        )?;

        let unbonding_batches: amulet_cw::vault::ActiveUnbondingsResponse = query_contract(
            &app,
            &native_pos_address,
            &amulet_cw::vault::QueryMsg::ActiveUnbondings {
                address: None,
                limit: None,
            },
        )?;

        assert_eq!(unbonding_batches.unbondings.len(), i + 1);

        let unbondings = query_staking_unbondings(&app, &native_pos_address)?;

        assert!(unbondings.iter().all(|res| res.entries.len() == i + 1
            && res.entries[i].amount >= alice_withdraw_amount / INITIAL_VAULT_SET_SIZE as u128));
    }

    // withdraw again and the batch should be pending
    execute_contract(
        &app,
        &alice,
        &hub_address,
        &amulet_cw::hub::UserMsg::Withdraw {
            vault: native_pos_address.clone(),
            amount: (alice_withdraw_amount * 2).into(),
        },
        &[],
    )?;

    let pending_batch: amulet_cw::vault::PendingUnbondingResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::PendingUnbonding { address: None },
    )?;

    assert_eq!(pending_batch.amount.u128(), alice_withdraw_amount * 2);

    app.increase_time((unbonding_period / max_unbonding_entries) + 1);

    execute_contract(
        &app,
        &alice,
        &native_pos_address,
        &amulet_cw::vault::ExecuteMsg::StartUnbond {},
        &[],
    )?;

    let unbonding_batches: amulet_cw::vault::ActiveUnbondingsResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::ActiveUnbondings {
            address: None,
            limit: None,
        },
    )?;

    assert_eq!(
        unbonding_batches.unbondings.len(),
        max_unbonding_entries as usize
    );

    // unbondings batches returned in descending order (newest first)
    assert_eq!(
        unbonding_batches.unbondings[0].amount.u128(),
        alice_withdraw_amount * 2
    );

    assert_eq!(
        unbonding_batches.unbondings[0].end,
        app.get_block_timestamp()
            .plus_seconds(unbonding_period)
            .seconds()
    );

    // alice's first withdrawal should be claimable
    let alice_pre_claim_claimable: amulet_cw::vault::ClaimableResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::Claimable {
            address: alice.address(),
        },
    )?;

    assert_eq!(
        alice_pre_claim_claimable.amount.u128(),
        alice_withdraw_amount
    );

    let native_pos_pre_claim_balance =
        query_bank_balance(&app, &native_pos_address, &staking_params.bond_denom)?;

    assert_eq!(native_pos_pre_claim_balance, alice_withdraw_amount);

    let alice_pre_claim_balance =
        query_bank_balance(&app, alice.address().as_str(), &staking_params.bond_denom)?;

    execute_contract(
        &app,
        &alice,
        &native_pos_address,
        &amulet_cw::vault::ExecuteMsg::Claim {},
        &[],
    )?;

    let alice_post_claim_claimable: amulet_cw::vault::ClaimableResponse = query_contract(
        &app,
        &native_pos_address,
        &amulet_cw::vault::QueryMsg::Claimable {
            address: alice.address(),
        },
    )?;

    assert_eq!(alice_post_claim_claimable.amount.u128(), 0);

    let native_pos_pre_claim_balance =
        query_bank_balance(&app, &native_pos_address, &staking_params.bond_denom)?;

    assert_eq!(native_pos_pre_claim_balance, 0);

    let alice_post_claim_balance =
        query_bank_balance(&app, alice.address().as_str(), &staking_params.bond_denom)?;

    assert!(alice_post_claim_balance > alice_pre_claim_balance);

    let native_pos_metadata: amulet_native_pos::msg::Metadata = query_contract(
        &app,
        &native_pos_address,
        &amulet_native_pos::msg::StrategyQueryMsg::Metadata {},
    )?;

    assert_eq!(native_pos_metadata.available_to_claim.u128(), 0);
    assert_eq!(native_pos_metadata.last_acknowledged_batch, Some(0));

    Ok(())
}

// let mint_params: osmosis_std::types::osmosis::mint::v1beta1::QueryParamsResponse = app.query(
//     "/osmosis.mint.v1beta1.Query/Params",
//     &osmosis_std::types::osmosis::mint::v1beta1::QueryParamsRequest {},
// )?;

// let mint_params = mint_params
//     .params
//     .ok_or_else(|| anyhow!("No params returned for x/mint module"))?;

// let mint_current_epoch: osmosis_std::types::osmosis::epochs::v1beta1::QueryCurrentEpochResponse = app
//     .query(
//         "/osmosis.epochs.v1beta1.Query/CurrentEpoch",
//         &osmosis_std::types::osmosis::epochs::v1beta1::QueryCurrentEpochRequest {
//             identifier: mint_params.epoch_identifier.clone(),
//         },
//     )?;

// let total_pending_rewards: osmosis_std::types::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsResponse =
//     app.query(
//         "/cosmos.distribution.v1beta1.Query/DelegationTotalRewards",
//         &osmosis_std::types::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsRequest {
//             delegator_address: native_pos_address.clone(),
//         },
//     )?;

// let total_pending_rewards: u128 = total_pending_rewards.total[0]
//     .amount
//     .parse::<Decimal>()?
//     .to_uint_floor()
//     .u128();

// execute_contract(
//     &app,
//     &bob,
//     &native_pos_address,
//     &amulet_native_pos::msg::StrategyExecuteMsg::CollectRewards {},
//     &[],
// )?;

// let expected_total_delegations = alice_deposit_amount + total_pending_rewards;

// let all_delegations = query_staking_delegations(&app, &native_pos_address)?;

// assert_eq!(all_delegations.len(), INITIAL_VAULT_SET_SIZE);
// assert!(all_delegations
//     .iter()
//     .all(|(_, amount)| *amount >= expected_total_delegations / INITIAL_VAULT_SET_SIZE as u128));
// let total_delegations = all_delegations
//     .iter()
//     .map(|(_, amount)| amount)
//     .sum::<u128>();
// assert_eq!(
//     total_delegations, expected_total_delegations,
//     "{total_delegations} == {expected_total_delegations}"
// );
