use cosmwasm_std::{
    coins, from_json,
    testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
    to_json_binary, Addr, Binary, ContractResult, Empty, MessageInfo, OwnedDeps, SystemResult,
    WasmQuery,
};
use test_utils::prelude::*;

use amulet_cw::{
    strategy::generic_lst::{QueryMsg as RedemptionRateOracleQuery, RedemptionRateResponse},
    vault::{
        ActiveUnbondingsResponse, ClaimableResponse, DepositResponse, PendingUnbondingResponse,
        QueryMsg as VaultQueryMsg, StateResponse, UnbondingLogMetadata,
    },
};

use crate::msg::{
    AdminExecuteMsg, AdminQueryMsg, InstantiateMsg, MetadataResponse, StrategyExecuteMsg,
    StrategyQueryMsg, VaultExecuteMsg,
};

use super::{execute, instantiate, query};

const REDEMPTION_RATE_ORACLE: &str = "redemption_rate_oracle";
const LST: &str = "liquid_staking_token";

type MockDeps = OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>;

macro_rules! info {
    ($sender:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: vec![],
        }
    };
    ($sender:literal, $amount:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: coins($amount, LST),
        }
    };
    ($sender:literal, $amount:literal, $asset:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: coins($amount, $asset),
        }
    };
}

fn update_querier(deps: &mut MockDeps, rate: f64) {
    deps.querier.update_wasm(move |query| {
        let WasmQuery::Smart { msg, contract_addr } = query else {
            panic!("unexpected wasm query: {query:?}");
        };

        let binary = match contract_addr.as_str() {
            REDEMPTION_RATE_ORACLE => match from_json(msg).unwrap() {
                RedemptionRateOracleQuery::RedemptionRate {} => {
                    to_json_binary(&RedemptionRateResponse {
                        rate: rate.to_string().parse().unwrap(),
                    })
                }
            },
            _ => panic!("unexpected contract query addr: {contract_addr}"),
        }
        .unwrap();

        SystemResult::Ok(ContractResult::Ok(binary))
    });
}

fn init() -> MockDeps {
    let mut deps = mock_dependencies();

    update_querier(&mut deps, 1.0);

    instantiate(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        InstantiateMsg {
            lst_redemption_rate_oracle: REDEMPTION_RATE_ORACLE.into(),
            lst_denom: LST.into(),
            lst_decimals: 6,
            underlying_decimals: 6,
        },
    )
    .unwrap();

    deps
}

fn into_json_string(bin: Binary) -> String {
    String::from_utf8(bin.0).unwrap()
}

#[test]
fn deposit() {
    let mut deps = init();

    update_querier(&mut deps, 1.1);

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 1_000),
        VaultExecuteMsg::Deposit {}.into(),
    )
    .unwrap();

    check(
        &response,
        expect![[r#"
        (
          messages: [
            (
              id: 0,
              msg: custom(mint_tokens(
                denom: "factory/cosmos2contract/share",
                amount: "1100000000000000",
                mint_to_address: "bob",
              )),
              gas_limit: None,
              reply_on: never,
            ),
          ],
          attributes: [],
          events: [],
          data: Some("eyJ0b3RhbF9zaGFyZXNfaXNzdWVkIjoiMTEwMDAwMDAwMDAwMDAwMCIsInRvdGFsX2RlcG9zaXRzX3ZhbHVlIjoiMTEwMCIsIm1pbnRlZF9zaGFyZXMiOiIxMTAwMDAwMDAwMDAwMDAwIiwiZGVwb3NpdF92YWx1ZSI6IjExMDAifQ=="),
        )"#]],
    );

    check(
        response
            .data
            .map(from_json::<DepositResponse>)
            .unwrap()
            .unwrap(),
        expect![[r#"
            (
              total_shares_issued: "1100000000000000",
              total_deposits_value: "1100",
              minted_shares: "1100000000000000",
              deposit_value: "1100",
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              lst_redemption_rate_oracle: "redemption_rate_oracle",
              lst_denom: "liquid_staking_token",
              lst_decimals: 6,
              underlying_decimals: 6,
              active_lst_balance: "1000",
              claimable_lst_balance: "0",
            )"#]],
    );

    check(
        query(deps.as_ref(), mock_env(), VaultQueryMsg::State {}.into())
            .map(from_json::<StateResponse>)
            .unwrap()
            .unwrap(),
        expect![[r#"
            (
              total_deposits: "1100",
              total_issued_shares: "1100000000000000",
            )"#]],
    );
}

#[test]
fn donate() {
    let mut deps = init();

    update_querier(&mut deps, 1.1);

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 1_000),
        VaultExecuteMsg::Donate {}.into(),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              lst_redemption_rate_oracle: "redemption_rate_oracle",
              lst_denom: "liquid_staking_token",
              lst_decimals: 6,
              underlying_decimals: 6,
              active_lst_balance: "1000",
              claimable_lst_balance: "0",
            )"#]],
    );

    check(
        query(deps.as_ref(), mock_env(), VaultQueryMsg::State {}.into())
            .map(from_json::<StateResponse>)
            .unwrap()
            .unwrap(),
        expect![[r#"
            (
              total_deposits: "1100",
              total_issued_shares: "0",
            )"#]],
    );
}

#[test]
fn redeem() {
    let mut deps = init();

    update_querier(&mut deps, 1.1);

    execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 1_000),
        VaultExecuteMsg::Deposit {}.into(),
    )
    .unwrap();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 550_000_000_000_000, "factory/cosmos2contract/share"),
        VaultExecuteMsg::Redeem {
            recipient: "bob".into(),
        }
        .into(),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: custom(burn_tokens(
                    denom: "factory/cosmos2contract/share",
                    amount: "550000000000000",
                    burn_from_address: "",
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              lst_redemption_rate_oracle: "redemption_rate_oracle",
              lst_denom: "liquid_staking_token",
              lst_decimals: 6,
              underlying_decimals: 6,
              active_lst_balance: "501",
              claimable_lst_balance: "499",
            )"#]],
    );

    check(
        query(deps.as_ref(), mock_env(), VaultQueryMsg::State {}.into())
            .map(from_json::<StateResponse>)
            .unwrap()
            .unwrap(),
        expect![[r#"
            (
              total_deposits: "551",
              total_issued_shares: "550000000000000",
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::Claimable {
                address: "bob".into(),
            }
            .into(),
        )
        .map(from_json::<ClaimableResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              amount: "499",
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::UnbondingLogMetadata {
                address: "bob".into(),
            }
            .into(),
        )
        .map(from_json::<UnbondingLogMetadata>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              last_committed_batch_id: Some(0),
              first_entered_batch: Some(0),
              last_entered_batch: Some(0),
              last_claimed_batch: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::ActiveUnbondings {
                address: Some("bob".into()),
            }
            .into(),
        )
        .map(from_json::<ActiveUnbondingsResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              unbondings: [
                (
                  amount: "550",
                  start: 1571797419,
                  end: 1571797419,
                ),
              ],
            )"#]],
    );
}

#[test]
fn start_unbond_errs() {
    let mut deps = init();

    update_querier(&mut deps, 1.1);

    execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 1_000),
        VaultExecuteMsg::Deposit {}.into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 550_000_000_000_000, "factory/cosmos2contract/share"),
        VaultExecuteMsg::Redeem {
            recipient: "bob".into(),
        }
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::PendingUnbonding { address: None }.into(),
        )
        .map(from_json::<PendingUnbondingResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              amount: "0",
              start_hint: None,
            )"#]],
    );

    check(
        execute(
            deps.as_mut(),
            mock_env(),
            info!("bob"),
            VaultExecuteMsg::StartUnbond {}.into(),
        )
        .unwrap_err()
        .to_string(),
        expect![[r#""nothing to unbond""#]],
    )
}

#[test]
fn claim() {
    let mut deps = init();

    update_querier(&mut deps, 1.1);

    execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 1_000),
        VaultExecuteMsg::Deposit {}.into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 550_000_000_000_000, "factory/cosmos2contract/share"),
        VaultExecuteMsg::Redeem {
            recipient: "bob".into(),
        }
        .into(),
    )
    .unwrap();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("bob"),
        VaultExecuteMsg::Claim {}.into(),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: bank(send(
                    to_address: "bob",
                    amount: [
                      (
                        denom: "liquid_staking_token",
                        amount: "499",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              lst_redemption_rate_oracle: "redemption_rate_oracle",
              lst_denom: "liquid_staking_token",
              lst_decimals: 6,
              underlying_decimals: 6,
              active_lst_balance: "501",
              claimable_lst_balance: "0",
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::Claimable {
                address: "bob".into(),
            }
            .into(),
        )
        .map(from_json::<ClaimableResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              amount: "0",
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            VaultQueryMsg::UnbondingLogMetadata {
                address: "bob".into(),
            }
            .into(),
        )
        .map(from_json::<UnbondingLogMetadata>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              last_committed_batch_id: Some(0),
              first_entered_batch: Some(0),
              last_entered_batch: Some(0),
              last_claimed_batch: Some(0),
            )"#]],
    );
}

#[test]
fn set_redemption_rate_oracle_non_admin_errs() {
    let mut deps = init();

    check(
        execute(
            deps.as_mut(),
            mock_env(),
            info!("average_joe"),
            StrategyExecuteMsg::SetRedemptionRateOracle {
                oracle: "a_different_oracle".into(),
            }
            .into(),
        )
        .unwrap_err()
        .to_string(),
        expect![[r#""unauthorized""#]],
    )
}

#[test]
fn initialisation() {
    check(
        query(
            init().as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              lst_redemption_rate_oracle: "redemption_rate_oracle",
              lst_denom: "liquid_staking_token",
              lst_decimals: 6,
              underlying_decimals: 6,
              active_lst_balance: "0",
              claimable_lst_balance: "0",
            )"#]],
    );
}

#[test]
fn set_redemption_rate_oracle() {
    let mut deps = init();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap()
        .lst_redemption_rate_oracle,
        expect![[r#""redemption_rate_oracle""#]],
    );

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        StrategyExecuteMsg::SetRedemptionRateOracle {
            oracle: "a_different_oracle".into(),
        }
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            StrategyQueryMsg::Metadata {}.into(),
        )
        .map(from_json::<MetadataResponse>)
        .unwrap()
        .unwrap()
        .lst_redemption_rate_oracle,
        expect![[r#""a_different_oracle""#]],
    );
}

#[test]
fn admin() {
    let mut deps = init();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            AdminQueryMsg::CurrentAdmin {}.into(),
        )
        .map(into_json_string)
        .unwrap(),
        expect![[r#""{\"current_admin\":null}""#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            AdminQueryMsg::PendingAdmin {}.into(),
        )
        .map(into_json_string)
        .unwrap(),
        expect![[r#""{\"pending_admin\":null}""#]],
    );

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        AdminExecuteMsg::TransferAdminRole {
            next_admin: "admin_one".into(),
        }
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            AdminQueryMsg::PendingAdmin {}.into(),
        )
        .map(into_json_string)
        .unwrap(),
        expect![[r#""{\"pending_admin\":\"admin_one\"}""#]],
    );

    execute(
        deps.as_mut(),
        mock_env(),
        info!("admin_one"),
        AdminExecuteMsg::ClaimAdminRole {}.into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            AdminQueryMsg::CurrentAdmin {}.into(),
        )
        .map(into_json_string)
        .unwrap(),
        expect![[r#""{\"current_admin\":\"admin_one\"}""#]],
    );

    execute(
        deps.as_mut(),
        mock_env(),
        info!("admin_one"),
        AdminExecuteMsg::TransferAdminRole {
            next_admin: "admin_two".into(),
        }
        .into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("admin_one"),
        AdminExecuteMsg::CancelRoleTransfer {}.into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            AdminQueryMsg::PendingAdmin {}.into(),
        )
        .map(into_json_string)
        .unwrap(),
        expect![[r#""{\"pending_admin\":\"admin_one\"}""#]],
    );
}
