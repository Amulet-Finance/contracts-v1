use cosmwasm_std::{
    coins, from_json,
    testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
    Addr, Binary, Empty, MessageInfo, OwnedDeps,
};
use test_utils::prelude::*;

use amulet_cw::mint::{AllAssetsResponse, Metadata, WhitelistedResponse};

use crate::msg::{AdminExecuteMsg, AdminQueryMsg, InstantiateMsg, MintExecuteMsg, MintQueryMsg};

use super::{execute, instantiate, query};

type MockDeps = OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>;

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

fn init() -> MockDeps {
    let mut deps = mock_dependencies();

    instantiate(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        InstantiateMsg {
            token_factory_flavour: amulet_token_factory::Flavour::Osmosis,
        },
    )
    .unwrap();

    deps
}

fn into_json_string(bin: Binary) -> String {
    String::from_utf8(bin.0).unwrap()
}

#[test]
fn create_synthetic() {
    let mut deps = init();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::CreateSynthetic {
            ticker: "SYNTH".into(),
            decimals: 6,
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
                  msg: custom(create_denom(
                    subdenom: "synth",
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
                (
                  id: 0,
                  msg: custom(set_denom_metadata(
                    description: "",
                    denom_units: [
                      (
                        denom: "factory/cosmos2contract/synth",
                        exponent: 0,
                        aliases: [],
                      ),
                      (
                        denom: "SYNTH",
                        exponent: 6,
                        aliases: [],
                      ),
                    ],
                    base: "factory/cosmos2contract/synth",
                    display: "SYNTH",
                    name: "SYNTH",
                    symbol: "SYNTH",
                    uri: "",
                    uri_hash: "",
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
            MintQueryMsg::AllAssets {
                page: None,
                limit: None,
            }
            .into(),
        )
        .map(from_json::<AllAssetsResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              assets: [
                (
                  denom: "factory/cosmos2contract/synth",
                  ticker: "synth",
                  decimals: 6,
                ),
              ],
              total_count: 1,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            MintQueryMsg::Synthetic {
                denom: "factory/cosmos2contract/synth".into(),
            }
            .into(),
        )
        .map(from_json::<Metadata>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              denom: "factory/cosmos2contract/synth",
              ticker: "synth",
              decimals: 6,
            )"#]],
    );
}

#[test]
fn set_whitelisted() {
    let mut deps = init();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::SetWhitelisted {
            minter: "minter".into(),
            whitelisted: true,
        }
        .into(),
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
            MintQueryMsg::Whitelisted {
                minter: "minter".into(),
            }
            .into(),
        )
        .map(from_json::<WhitelistedResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              whitelisted: true,
            )"#]],
    );

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::SetWhitelisted {
            minter: "minter".into(),
            whitelisted: false,
        }
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            MintQueryMsg::Whitelisted {
                minter: "minter".into(),
            }
            .into(),
        )
        .map(from_json::<WhitelistedResponse>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              whitelisted: false,
            )"#]],
    );
}

#[test]
fn mint() {
    let mut deps = init();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::SetWhitelisted {
            minter: "minter".into(),
            whitelisted: true,
        }
        .into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::CreateSynthetic {
            ticker: "SYNTH".into(),
            decimals: 6,
        }
        .into(),
    )
    .unwrap();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("minter"),
        MintExecuteMsg::Mint {
            synthetic: "factory/cosmos2contract/synth".into(),
            amount: 1_000u128.into(),
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
                  msg: custom(mint_tokens(
                    denom: "factory/cosmos2contract/synth",
                    amount: "1000",
                    mint_to_address: "bob",
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
            MintQueryMsg::Synthetic {
                denom: "factory/cosmos2contract/synth".into(),
            }
            .into(),
        )
        .map(from_json::<Metadata>)
        .unwrap()
        .unwrap(),
        expect![[r#"
            (
              denom: "factory/cosmos2contract/synth",
              ticker: "synth",
              decimals: 6,
            )"#]],
    );
}

#[test]
fn burn() {
    let mut deps = init();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::SetWhitelisted {
            minter: "minter".into(),
            whitelisted: true,
        }
        .into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        MintExecuteMsg::CreateSynthetic {
            ticker: "SYNTH".into(),
            decimals: 6,
        }
        .into(),
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("minter"),
        MintExecuteMsg::Mint {
            synthetic: "factory/cosmos2contract/synth".into(),
            amount: 1_000u128.into(),
            recipient: "bob".into(),
        }
        .into(),
    )
    .unwrap();

    let response = execute(
        deps.as_mut(),
        mock_env(),
        info!("bob", 500, "factory/cosmos2contract/synth"),
        MintExecuteMsg::Burn {}.into(),
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
                    denom: "factory/cosmos2contract/synth",
                    amount: "500",
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
