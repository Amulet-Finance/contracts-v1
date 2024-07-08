use amulet_core::vault::SHARES_DECIMAL_PLACES;
use cosmwasm_schema::serde::de::DeserializeOwned;
use cosmwasm_std::{
    coins, from_json,
    testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage},
    to_json_binary, Addr, Binary, ContractResult, Empty, MessageInfo, OwnedDeps, Reply, Response,
    SubMsgResponse, SystemResult, WasmMsg, WasmQuery,
};
use prost::Message;
use test_utils::prelude::*;

use amulet_cw::{
    hub::{
        vault_registry::{DEPOSIT_REPLY_ID, MINT_REPLY_ID, REPAY_UNDERLYING_REPLY_ID},
        AdminMsg as HubAdminMsg, ListVaultsResponse, PositionResponse, TreasuryResponse,
        UserMsg as HubUserMsg, VaultMetadata,
    },
    mint::{ExecuteMsg as MintExecuteMsg, Metadata as SynthMetadata, QueryMsg as MintQueryMsg},
    vault::{
        DepositAssetResponse, DepositResponse, ExecuteMsg as VaultExecuteMsg,
        QueryMsg as VaultQueryMsg, SharesAssetResponse, StateResponse,
        UnderlyingAssetDecimalsResponse,
    },
};

use crate::msg::{AdminExecuteMsg, AdminQueryMsg, HubExecuteMsg, HubQueryMsg, InstantiateMsg};

use super::{execute, instantiate, query, reply};

const VAULT: &str = "vault";
const VAULT_SHARE: &str = "vault_share";
const VAULT_DEPOSIT_ASSET: &str = "vault_deposit_asset";
const SYNTHETIC_MINT: &str = "synthetic_mint";
const SYNTHETIC_ASSET: &str = "synthetic_asset";

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
            funds: coins($amount, VAULT_DEPOSIT_ASSET),
        }
    };
    ($sender:literal, $amount:literal, $asset:literal) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: coins($amount, $asset),
        }
    };
    ($sender:literal, $amount:literal, $asset:ident) => {
        MessageInfo {
            sender: Addr::unchecked($sender),
            funds: coins($amount, $asset),
        }
    };
}

fn update_querier(deps: &mut MockDeps, total_deposits: u128, total_issued_shares: u128) {
    deps.querier.update_wasm(move |query| {
        let WasmQuery::Smart { msg, contract_addr } = query else {
            panic!("unexpected wasm query: {query:?}");
        };

        let binary = match contract_addr.as_str() {
            VAULT => match from_json(msg).unwrap() {
                VaultQueryMsg::State {} => to_json_binary(&StateResponse {
                    total_deposits: total_deposits.into(),
                    total_issued_shares: total_issued_shares.into(),
                }),
                VaultQueryMsg::UnderlyingAssetDecimals {} => {
                    to_json_binary(&UnderlyingAssetDecimalsResponse { decimals: 6 })
                }
                VaultQueryMsg::DepositAsset {} => to_json_binary(&DepositAssetResponse {
                    denom: VAULT_DEPOSIT_ASSET.into(),
                }),
                VaultQueryMsg::SharesAsset {} => to_json_binary(&SharesAssetResponse {
                    denom: VAULT_SHARE.into(),
                }),
                q => panic!("unexpected vault query: {q:?}"),
            },
            SYNTHETIC_MINT => match from_json(msg).unwrap() {
                MintQueryMsg::Synthetic { denom } => to_json_binary(&SynthMetadata {
                    denom,
                    ticker: "SYNTH".into(),
                    decimals: 6,
                }),
                q => panic!("unexpected vault query: {q:?}"),
            },
            _ => panic!("unexpected contract query addr: {contract_addr}"),
        }
        .unwrap();

        SystemResult::Ok(ContractResult::Ok(binary))
    });
}

const fn shares_amount(n: u128) -> u128 {
    n * 10u128.pow(SHARES_DECIMAL_PLACES)
}

fn vault_deposit_reply(
    id: u64,
    total_deposit_value: u128,
    total_issued_shares: u128,
    minted_shares: u128,
    deposit_value: u128,
) -> Reply {
    #[derive(Clone, PartialEq, prost::Message)]
    struct MsgExecuteContractResponse {
        #[prost(bytes, tag = "1")]
        pub data: ::prost::alloc::vec::Vec<u8>,
    }

    let data = to_json_binary(&DepositResponse {
        total_shares_issued: total_issued_shares.into(),
        total_deposits_value: total_deposit_value.into(),
        minted_shares: minted_shares.into(),
        deposit_value: deposit_value.into(),
    })
    .unwrap()
    .to_vec();

    let data = MsgExecuteContractResponse { data }.encode_to_vec().into();

    Reply {
        id,
        result: cosmwasm_std::SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(data),
        }),
    }
}

fn init_with_registered_vault() -> MockDeps {
    let mut deps = mock_dependencies();

    update_querier(&mut deps, 0, 0);

    instantiate(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        InstantiateMsg {
            synthetic_mint: SYNTHETIC_MINT.into(),
        },
    )
    .unwrap();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::RegisterVault {
            vault: VAULT.into(),
            synthetic: SYNTHETIC_ASSET.into(),
        })
        .into(),
    )
    .unwrap();

    deps
}

fn execute_msgs(deps: &mut MockDeps, msgs: &[(MessageInfo, HubExecuteMsg)]) -> Response {
    let mut response = Response::default();

    for (info, msg) in msgs {
        response = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone().into()).unwrap();
    }

    response
}

fn into_json_string(bin: Binary) -> String {
    String::from_utf8(bin.0).unwrap()
}

fn into_response<T: DeserializeOwned>(bin: Binary) -> T {
    from_json(bin).unwrap()
}

#[test]
fn deposit() {
    let mut deps = init_with_registered_vault();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 1,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJkZXBvc2l0Ijp7fX0=",
                    funds: [
                      (
                        denom: "vault_deposit_asset",
                        amount: "1000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: success,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "deposit",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_deposit_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_deposit_msg,
        expect![[r#"
        [
          deposit(),
        ]"#]],
    );

    let response = reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [
                (
                  key: "kind",
                  value: "vault_deposit_callback",
                ),
                (
                  key: "reason",
                  value: "deposit",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "minted_shares",
                  value: "1000000000000000000000",
                ),
                (
                  key: "deposit_value",
                  value: "1000",
                ),
                (
                  key: "collateral_shares",
                  value: "1000000000000000000000",
                ),
                (
                  key: "collateral_balance",
                  value: "1000",
                ),
                (
                  key: "account_collateral",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: false,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "1000000000000000000000",
              reserve_balance: "0",
              reserve_shares: "0",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "0",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn deposit_on_behalf() {
    let mut deps = init_with_registered_vault();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetProxyConfig {
                    vault: VAULT.into(),
                    deposit: Some("deposit_proxy".into()),
                    advance: None,
                    redeem: None,
                    mint: None,
                }),
            ),
            (
                info!("deposit_proxy", 1_000),
                HubExecuteMsg::from(HubUserMsg::DepositOnBehalf {
                    vault: VAULT.into(),
                    behalf_of: "bob".into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 1,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJkZXBvc2l0Ijp7fX0=",
                    funds: [
                      (
                        denom: "vault_deposit_asset",
                        amount: "1000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: success,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "deposit",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_deposit_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_deposit_msg,
        expect![[r#"
        [
          deposit(),
        ]"#]],
    );

    let response = reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [
                (
                  key: "kind",
                  value: "vault_deposit_callback",
                ),
                (
                  key: "reason",
                  value: "deposit",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "minted_shares",
                  value: "1000000000000000000000",
                ),
                (
                  key: "deposit_value",
                  value: "1000",
                ),
                (
                  key: "collateral_shares",
                  value: "1000000000000000000000",
                ),
                (
                  key: "collateral_balance",
                  value: "1000",
                ),
                (
                  key: "account_collateral",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "0",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn repay_underlying() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::Advance {
                    vault: VAULT.into(),
                    amount: 500u128.into(),
                }),
            ),
            (
                info!("bob", 400),
                HubExecuteMsg::from(HubUserMsg::RepayUnderlying {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 2,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJkZXBvc2l0Ijp7fX0=",
                    funds: [
                      (
                        denom: "vault_deposit_asset",
                        amount: "400",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: success,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "repay_underlying",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "400",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_deposit_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_deposit_msg,
        expect![[r#"
        [
          deposit(),
        ]"#]],
    );

    let response = reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            REPAY_UNDERLYING_REPLY_ID,
            1_400,
            shares_amount(1_400),
            shares_amount(400),
            400,
        ),
    )
    .unwrap();

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [
                (
                  key: "kind",
                  value: "vault_deposit_callback",
                ),
                (
                  key: "reason",
                  value: "repay_underlying",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "minted_shares",
                  value: "400000000000000000000",
                ),
                (
                  key: "deposit_value",
                  value: "400",
                ),
                (
                  key: "reserve_shares",
                  value: "400000000000000000000",
                ),
                (
                  key: "reserve_balance",
                  value: "400",
                ),
                (
                  key: "account_debt",
                  value: "100",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "1000000000000000000000",
              reserve_balance: "400",
              reserve_shares: "400000000000000000000",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "100",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn repay_synthetic() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::Advance {
                    vault: VAULT.into(),
                    amount: 500u128.into(),
                }),
            ),
            (
                info!("bob", 400, SYNTHETIC_ASSET),
                HubExecuteMsg::from(HubUserMsg::RepaySynthetic {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJidXJuIjp7fX0=",
                    funds: [
                      (
                        denom: "synthetic_asset",
                        amount: "400",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "repay_synthetic",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "400",
                ),
                (
                  key: "account_debt",
                  value: "100",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        mint_msg,
        expect![[r#"
            [
              burn(),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "100",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn advance() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    let response = execute_msgs(
        &mut deps,
        &[(
            info!("bob"),
            HubExecuteMsg::from(HubUserMsg::Advance {
                vault: VAULT.into(),
                amount: 500u128.into(),
            }),
        )],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJtaW50Ijp7InN5bnRoZXRpYyI6InN5bnRoZXRpY19hc3NldCIsImFtb3VudCI6IjUwMCIsInJlY2lwaWVudCI6ImJvYiJ9fQ==",
                    funds: [],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "advance",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "500",
                ),
                (
                  key: "account_debt",
                  value: "500",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        mint_msg,
        expect![[r#"
            [
              mint(
                synthetic: "synthetic_asset",
                amount: "500",
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "500",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn advance_on_behalf() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetProxyConfig {
                    vault: VAULT.into(),
                    deposit: None,
                    advance: Some("advance_proxy".into()),
                    redeem: None,
                    mint: None,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    let response = execute_msgs(
        &mut deps,
        &[(
            info!("advance_proxy"),
            HubExecuteMsg::from(HubUserMsg::AdvanceOnBehalf {
                vault: VAULT.into(),
                amount: 500u128.into(),
                behalf_of: "bob".into(),
            }),
        )],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJtaW50Ijp7InN5bnRoZXRpYyI6InN5bnRoZXRpY19hc3NldCIsImFtb3VudCI6IjUwMCIsInJlY2lwaWVudCI6ImJvYiJ9fQ==",
                    funds: [],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "advance",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "500",
                ),
                (
                  key: "account_debt",
                  value: "500",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        mint_msg,
        expect![[r#"
            [
              mint(
                synthetic: "synthetic_asset",
                amount: "500",
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "500",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn withdraw() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_000, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[(
            info!("bob"),
            HubExecuteMsg::from(HubUserMsg::Withdraw {
                vault: VAULT.into(),
                amount: 500u128.into(),
            }),
        )],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJyZWRlZW0iOnsicmVjaXBpZW50IjoiYm9iIn19",
                    funds: [
                      (
                        denom: "vault_share",
                        amount: "500000000000000000000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "withdraw",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "500",
                ),
                (
                  key: "collateral_shares",
                  value: "500000000000000000000",
                ),
                (
                  key: "collateral_balance",
                  value: "500",
                ),
                (
                  key: "account_collateral",
                  value: "500",
                ),
                (
                  key: "redeem_shares",
                  value: "500000000000000000000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_msg,
        expect![[r#"
            [
              redeem(
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: false,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "500",
              collateral_shares: "500000000000000000000",
              reserve_balance: "0",
              reserve_shares: "0",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "500",
              debt: "0",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn self_liquidate() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_000, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::Advance {
                    vault: VAULT.into(),
                    amount: 500u128.into(),
                }),
            ),
            (
                info!("bob", 600, SYNTHETIC_ASSET),
                HubExecuteMsg::from(HubUserMsg::RepaySynthetic {
                    vault: VAULT.into(),
                }),
            ),
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::SelfLiquidate {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJtaW50Ijp7InN5bnRoZXRpYyI6InN5bnRoZXRpY19hc3NldCIsImFtb3VudCI6IjEwMCIsInJlY2lwaWVudCI6ImJvYiJ9fQ==",
                    funds: [],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJyZWRlZW0iOnsicmVjaXBpZW50IjoiYm9iIn19",
                    funds: [
                      (
                        denom: "vault_share",
                        amount: "1000000000000000000000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "self_liquidate",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "collateral_shares",
                  value: "0",
                ),
                (
                  key: "collateral_balance",
                  value: "0",
                ),
                (
                  key: "account_collateral",
                  value: "0",
                ),
                (
                  key: "account_credit",
                  value: "0",
                ),
                (
                  key: "redeem_shares",
                  value: "1000000000000000000000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_msg: Vec<_> = response
        .messages
        .iter()
        .filter_map(|m| match &m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_msg,
        expect![[r#"
            [
              redeem(
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        mint_msg,
        expect![[r#"
            [
              mint(
                synthetic: "synthetic_asset",
                amount: "100",
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "0",
              collateral_shares: "0",
              reserve_balance: "0",
              reserve_shares: "0",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "0",
              debt: "0",
              credit: "0",
              sum_payment_ratio: "0.0",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn convert_credit() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[(
            info!("bob"),
            HubExecuteMsg::from(HubUserMsg::ConvertCredit {
                vault: VAULT.into(),
                amount: 89u128.into(),
            }),
        )],
    );

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [
                (
                  key: "kind",
                  value: "convert_credit",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "89",
                ),
                (
                  key: "collateral_shares",
                  value: "990000000000000000000",
                ),
                (
                  key: "collateral_balance",
                  value: "1088",
                ),
                (
                  key: "reserve_shares",
                  value: "909090909090909092",
                ),
                (
                  key: "reserve_balance",
                  value: "1",
                ),
                (
                  key: "treasury_shares",
                  value: "9090909090909090908",
                ),
                (
                  key: "spr",
                  value: "0.08999999999999999999999999999999",
                ),
                (
                  key: "account_collateral",
                  value: "1088",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: false,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1088",
              collateral_shares: "990000000000000000000",
              reserve_balance: "1",
              reserve_shares: "909090909090909092",
              treasury_shares: "9090909090909090908",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: Some((
                ratio: "30625413022884461711703714668859139031",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                account: "bob".into(),
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1088",
              debt: "0",
              credit: "0",
              sum_payment_ratio: "0.08999999999999999999999999999999",
              vault_loss_detected: false,
            )"#]],
    );
}

#[test]
fn redeem() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::Advance {
                    vault: VAULT.into(),
                    amount: 200u128.into(),
                }),
            ),
            (
                info!("bob", 90, SYNTHETIC_ASSET),
                HubExecuteMsg::from(HubUserMsg::Redeem {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJyZWRlZW0iOnsicmVjaXBpZW50IjoiYm9iIn19",
                    funds: [
                      (
                        denom: "vault_share",
                        amount: "81818181818181818181",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJidXJuIjp7fX0=",
                    funds: [
                      (
                        denom: "synthetic_asset",
                        amount: "90",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "redeem",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "90",
                ),
                (
                  key: "reserve_shares",
                  value: "1",
                ),
                (
                  key: "reserve_balance",
                  value: "0",
                ),
                (
                  key: "redeem_shares",
                  value: "81818181818181818181",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_msg: Vec<_> = response
        .messages
        .iter()
        .filter_map(|m| match &m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_msg,
        expect![[r#"
            [
              redeem(
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        mint_msg,
        expect![[r#"
            [
              burn(),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "909090909090909090910",
              reserve_balance: "0",
              reserve_shares: "1",
              treasury_shares: "9090909090909090908",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: Some((
                ratio: "30625413022884461711703714668859139031",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn redeem_on_behalf() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetProxyConfig {
                    vault: VAULT.into(),
                    deposit: None,
                    advance: None,
                    redeem: Some("redeem_proxy".into()),
                    mint: None,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("bob"),
                HubExecuteMsg::from(HubUserMsg::Advance {
                    vault: VAULT.into(),
                    amount: 200u128.into(),
                }),
            ),
            (
                info!("redeem_proxy", 90, SYNTHETIC_ASSET),
                HubExecuteMsg::from(HubUserMsg::RedeemOnBehalf {
                    vault: VAULT.into(),
                    behalf_of: "bob".into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJyZWRlZW0iOnsicmVjaXBpZW50IjoiYm9iIn19",
                    funds: [
                      (
                        denom: "vault_share",
                        amount: "81818181818181818181",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
                (
                  id: 0,
                  msg: wasm(execute(
                    contract_addr: "synthetic_mint",
                    msg: "eyJidXJuIjp7fX0=",
                    funds: [
                      (
                        denom: "synthetic_asset",
                        amount: "90",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "redeem",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "90",
                ),
                (
                  key: "reserve_shares",
                  value: "1",
                ),
                (
                  key: "reserve_balance",
                  value: "0",
                ),
                (
                  key: "redeem_shares",
                  value: "81818181818181818181",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_msg: Vec<_> = response
        .messages
        .iter()
        .filter_map(|m| match &m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_msg,
        expect![[r#"
            [
              redeem(
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        mint_msg,
        expect![[r#"
            [
              burn(),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "909090909090909090910",
              reserve_balance: "0",
              reserve_shares: "1",
              treasury_shares: "9090909090909090908",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: Some((
                ratio: "30625413022884461711703714668859139031",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: Some("redeem_proxy"),
            )"#]],
    );
}

#[test]
fn mint() {
    let mut deps = init_with_registered_vault();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Mint {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 3,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJkZXBvc2l0Ijp7fX0=",
                    funds: [
                      (
                        denom: "vault_deposit_asset",
                        amount: "1000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: success,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "mint",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_deposit_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_deposit_msg,
        expect![[r#"
        [
          deposit(),
        ]"#]],
    );

    let response = reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            MINT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    check(
        &response,
        expect![[r#"
        (
          messages: [
            (
              id: 0,
              msg: wasm(execute(
                contract_addr: "synthetic_mint",
                msg: "eyJtaW50Ijp7InN5bnRoZXRpYyI6InN5bnRoZXRpY19hc3NldCIsImFtb3VudCI6IjEwMDAiLCJyZWNpcGllbnQiOiJib2IifX0=",
                funds: [],
              )),
              gas_limit: None,
              reply_on: never,
            ),
          ],
          attributes: [
            (
              key: "kind",
              value: "vault_deposit_callback",
            ),
            (
              key: "reason",
              value: "mint",
            ),
            (
              key: "vault",
              value: "vault",
            ),
            (
              key: "recipient",
              value: "bob",
            ),
            (
              key: "minted_shares",
              value: "1000000000000000000000",
            ),
            (
              key: "deposit_value",
              value: "1000",
            ),
            (
              key: "reserve_shares",
              value: "1000000000000000000000",
            ),
            (
              key: "reserve_balance",
              value: "1000",
            ),
          ],
          events: [],
          data: None,
        )"#]],
    );

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        mint_msg,
        expect![[r#"
            [
              mint(
                synthetic: "synthetic_asset",
                amount: "1000",
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: false,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "0",
              collateral_shares: "0",
              reserve_balance: "1000",
              reserve_shares: "1000000000000000000000",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn mint_on_behalf() {
    let mut deps = init_with_registered_vault();

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetProxyConfig {
                    vault: VAULT.into(),
                    deposit: None,
                    advance: None,
                    redeem: None,
                    mint: Some("mint_proxy".into()),
                }),
            ),
            (
                info!("mint_proxy", 1_000),
                HubExecuteMsg::from(HubUserMsg::MintOnBehalf {
                    vault: VAULT.into(),
                    behalf_of: "bob".into(),
                }),
            ),
        ],
    );

    check(
        &response,
        expect![[r#"
            (
              messages: [
                (
                  id: 3,
                  msg: wasm(execute(
                    contract_addr: "vault",
                    msg: "eyJkZXBvc2l0Ijp7fX0=",
                    funds: [
                      (
                        denom: "vault_deposit_asset",
                        amount: "1000",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: success,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "mint",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "recipient",
                  value: "bob",
                ),
                (
                  key: "amount",
                  value: "1000",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    let vault_deposit_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<VaultExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        vault_deposit_msg,
        expect![[r#"
        [
          deposit(),
        ]"#]],
    );

    let response = reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            MINT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    check(
        &response,
        expect![[r#"
        (
          messages: [
            (
              id: 0,
              msg: wasm(execute(
                contract_addr: "synthetic_mint",
                msg: "eyJtaW50Ijp7InN5bnRoZXRpYyI6InN5bnRoZXRpY19hc3NldCIsImFtb3VudCI6IjEwMDAiLCJyZWNpcGllbnQiOiJib2IifX0=",
                funds: [],
              )),
              gas_limit: None,
              reply_on: never,
            ),
          ],
          attributes: [
            (
              key: "kind",
              value: "vault_deposit_callback",
            ),
            (
              key: "reason",
              value: "mint",
            ),
            (
              key: "vault",
              value: "vault",
            ),
            (
              key: "recipient",
              value: "bob",
            ),
            (
              key: "minted_shares",
              value: "1000000000000000000000",
            ),
            (
              key: "deposit_value",
              value: "1000",
            ),
            (
              key: "reserve_shares",
              value: "1000000000000000000000",
            ),
            (
              key: "reserve_balance",
              value: "1000",
            ),
          ],
          events: [],
          data: None,
        )"#]],
    );

    let mint_msg: Vec<_> = response
        .messages
        .into_iter()
        .filter_map(|m| match m.msg {
            cosmwasm_std::CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
                from_json::<MintExecuteMsg>(msg).ok()
            }
            _ => None,
        })
        .collect();

    check(
        mint_msg,
        expect![[r#"
            [
              mint(
                synthetic: "synthetic_asset",
                amount: "1000",
                recipient: "bob",
              ),
            ]"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: false,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "0",
              collateral_shares: "0",
              reserve_balance: "1000",
              reserve_shares: "1000000000000000000000",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: None,
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: Some("mint_proxy"),
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn claim_treasury() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetTreasury {
                    address: "treasury".into(),
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("anyone"),
                HubExecuteMsg::from(HubUserMsg::Evaluate {
                    vault: VAULT.into(),
                }),
            ),
            (
                info!("treasury"),
                HubExecuteMsg::from(HubUserMsg::ClaimTreasury {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: bank(send(
                    to_address: "treasury",
                    amount: [
                      (
                        denom: "vault_share",
                        amount: "9090909090909090908",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "claim_treasury",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "treasury_shares",
                  value: "0",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "909090909090909090910",
              reserve_balance: "90",
              reserve_shares: "81818181818181818182",
              treasury_shares: "0",
              amo: None,
              amo_allocation: 0,
              amo_shares: "0",
              sum_payment_ratio: Some((
                ratio: "30625413022884461711703714668859139031",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn claim_amo() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAmo {
                    vault: VAULT.into(),
                    amo: "amo".into(),
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAmoAllocation {
                    vault: VAULT.into(),
                    bps: 1_000,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[
            (
                info!("anyone"),
                HubExecuteMsg::from(HubUserMsg::Evaluate {
                    vault: VAULT.into(),
                }),
            ),
            (
                info!("amo"),
                HubExecuteMsg::from(HubUserMsg::ClaimAmo {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    check(
        response,
        expect![[r#"
            (
              messages: [
                (
                  id: 0,
                  msg: bank(send(
                    to_address: "amo",
                    amount: [
                      (
                        denom: "vault_share",
                        amount: "8181818181818181818",
                      ),
                    ],
                  )),
                  gas_limit: None,
                  reply_on: never,
                ),
              ],
              attributes: [
                (
                  key: "kind",
                  value: "claim_amo",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "amo_shares",
                  value: "0",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "909090909090909090910",
              reserve_balance: "81",
              reserve_shares: "73636363636363636364",
              treasury_shares: "9090909090909090908",
              amo: Some("amo"),
              amo_allocation: 1000,
              amo_shares: "0",
              sum_payment_ratio: Some((
                ratio: "30285130655963523248240340061427370819",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn evaluate() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAmo {
                    vault: VAULT.into(),
                    amo: "amo".into(),
                }),
            ),
            (
                info!("creator"),
                HubExecuteMsg::from(HubAdminMsg::SetAmoAllocation {
                    vault: VAULT.into(),
                    bps: 1_000,
                }),
            ),
            (
                info!("bob", 1_000),
                HubExecuteMsg::from(HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1_100, shares_amount(1_000));

    let response = execute_msgs(
        &mut deps,
        &[(
            info!("anyone"),
            HubExecuteMsg::from(HubUserMsg::Evaluate {
                vault: VAULT.into(),
            }),
        )],
    );

    check(
        response,
        expect![[r#"
            (
              messages: [],
              attributes: [
                (
                  key: "kind",
                  value: "evaluate",
                ),
                (
                  key: "vault",
                  value: "vault",
                ),
                (
                  key: "account",
                  value: "anyone",
                ),
                (
                  key: "collateral_shares",
                  value: "909090909090909090910",
                ),
                (
                  key: "reserve_shares",
                  value: "73636363636363636364",
                ),
                (
                  key: "reserve_balance",
                  value: "81",
                ),
                (
                  key: "treasury_shares",
                  value: "9090909090909090908",
                ),
                (
                  key: "amo_shares",
                  value: "8181818181818181818",
                ),
                (
                  key: "spr",
                  value: "0.08899999999999999999999999999999",
                ),
              ],
              events: [],
              data: None,
            )"#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap(),
        expect![[r#"
            (
              vault: "vault",
              synthetic: "synthetic_asset",
              deposit_enabled: true,
              advance_enabled: true,
              max_ltv_bps: 5000,
              collateral_yield_fee_bps: 1000,
              reserve_yield_fee_bps: 10000,
              fixed_advance_fee_bps: 25,
              advance_fee_recipient: None,
              advance_fee_oracle: None,
              collateral_balance: "1000",
              collateral_shares: "909090909090909090910",
              reserve_balance: "81",
              reserve_shares: "73636363636363636364",
              treasury_shares: "9090909090909090908",
              amo: Some("amo"),
              amo_allocation: 1000,
              amo_shares: "8181818181818181818",
              sum_payment_ratio: Some((
                ratio: "30285130655963523248240340061427370819",
                timestamp: 1571797419,
              )),
              deposit_proxy: None,
              advance_proxy: None,
              mint_proxy: None,
              redeem_proxy: None,
            )"#]],
    );
}

#[test]
fn position_query_vault_loss_detected() {
    let mut deps = init_with_registered_vault();

    execute_msgs(
        &mut deps,
        &[
            (
                info!("creator"),
                HubAdminMsg::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }
                .into(),
            ),
            (
                info!("creator"),
                HubAdminMsg::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true,
                }
                .into(),
            ),
            (
                info!("bob", 1_000),
                HubUserMsg::Deposit {
                    vault: VAULT.into(),
                }
                .into(),
            ),
        ],
    );

    reply(
        deps.as_mut(),
        mock_env(),
        vault_deposit_reply(
            DEPOSIT_REPLY_ID,
            1_000,
            shares_amount(1_000),
            shares_amount(1_000),
            1_000,
        ),
    )
    .unwrap();

    update_querier(&mut deps, 1100, shares_amount(1_000));

    execute_msgs(
        &mut deps,
        &[(
            info!("anyone"),
            HubUserMsg::Evaluate {
                vault: VAULT.into(),
            }
            .into(),
        )],
    );

    update_querier(&mut deps, 1050, shares_amount(1_000));

    check(
        execute(
            deps.as_mut(),
            mock_env(),
            info!("bob"),
            HubExecuteMsg::from(HubUserMsg::Evaluate {
                vault: VAULT.into(),
            })
            .into(),
        )
        .unwrap_err()
        .to_string(),
        expect![[r#""vault shares have suffered a loss in value""#]],
    );

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::Position {
                vault: VAULT.into(),
                account: "bob".into(),
            }
            .into(),
        )
        .map(into_response::<PositionResponse>)
        .unwrap(),
        expect![[r#"
            (
              collateral: "1000",
              debt: "0",
              credit: "89",
              sum_payment_ratio: "0.08999999999999999999999999999999",
              vault_loss_detected: true,
            )"#]],
    );
}

#[test]
fn register_vault() {
    let deps = init_with_registered_vault();

    check(
        query(deps.as_ref(), mock_env(), HubQueryMsg::ListVaults {}.into())
            .map(into_response::<ListVaultsResponse>)
            .unwrap(),
        expect![[r#"
            (
              vaults: [
                (
                  vault: "vault",
                  synthetic: "synthetic_asset",
                  deposit_enabled: false,
                  advance_enabled: false,
                  max_ltv_bps: 5000,
                  collateral_yield_fee_bps: 1000,
                  reserve_yield_fee_bps: 10000,
                  fixed_advance_fee_bps: 25,
                  advance_fee_recipient: None,
                  advance_fee_oracle: None,
                  collateral_balance: "0",
                  collateral_shares: "0",
                  reserve_balance: "0",
                  reserve_shares: "0",
                  treasury_shares: "0",
                  amo: None,
                  amo_allocation: 0,
                  amo_shares: "0",
                  sum_payment_ratio: None,
                  deposit_proxy: None,
                  advance_proxy: None,
                  mint_proxy: None,
                  redeem_proxy: None,
                ),
              ],
            )"#]],
    );
}

#[test]
fn set_treasury() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetTreasury {
            address: "treasury".into(),
        })
        .into(),
    )
    .unwrap();

    check(
        query(deps.as_ref(), mock_env(), HubQueryMsg::Treasury {}.into())
            .map(into_response::<TreasuryResponse>)
            .unwrap(),
        expect![[r#"
            (
              treasury: Some("treasury"),
            )"#]],
    );
}

#[test]
fn set_deposits_enabled() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetDepositsEnabled {
            vault: VAULT.into(),
            enabled: true,
        })
        .into(),
    )
    .unwrap();

    assert!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .deposit_enabled
    )
}

#[test]
fn set_advance_enabled() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetAdvanceEnabled {
            vault: VAULT.into(),
            enabled: true,
        })
        .into(),
    )
    .unwrap();

    assert!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .advance_enabled
    )
}

#[test]
fn set_max_ltv() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetMaxLtv {
            vault: VAULT.into(),
            bps: 9_000,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .max_ltv_bps,
        9_000
    )
}

#[test]
fn set_collateral_yield_fee() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetCollateralYieldFee {
            vault: VAULT.into(),
            bps: 9_000,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .collateral_yield_fee_bps,
        9_000
    )
}

#[test]
fn set_reserves_treasury_fee() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetReservesTreasuryFee {
            vault: VAULT.into(),
            bps: 9_000,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .reserve_yield_fee_bps,
        9_000
    )
}

#[test]
fn set_advance_fee_recipient() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetAdvanceFeeRecipient {
            vault: VAULT.into(),
            recipient: "treasury".into(),
        })
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .advance_fee_recipient,
        expect![[r#"Some("treasury")"#]],
    )
}

#[test]
fn set_fixed_advance_fee() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetFixedAdvanceFee {
            vault: VAULT.into(),
            bps: 100,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .fixed_advance_fee_bps,
        100
    )
}

#[test]
fn set_advance_fee_oracle() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetAdvanceFeeOracle {
            vault: VAULT.into(),
            oracle: "advance_fee_oracle".into(),
        })
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .advance_fee_oracle,
        expect![[r#"Some("advance_fee_oracle")"#]],
    )
}

#[test]
fn set_amo() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetAmo {
            vault: VAULT.into(),
            amo: "amo".into(),
        })
        .into(),
    )
    .unwrap();

    check(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .amo,
        expect![[r#"Some("amo")"#]],
    )
}

#[test]
fn set_amo_allocation() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetAmoAllocation {
            vault: VAULT.into(),
            bps: 1_000,
        })
        .into(),
    )
    .unwrap();

    assert_eq!(
        query(
            deps.as_ref(),
            mock_env(),
            HubQueryMsg::VaultMetadata {
                vault: VAULT.into(),
            }
            .into(),
        )
        .map(into_response::<VaultMetadata>)
        .unwrap()
        .amo_allocation,
        1_000
    )
}

#[test]
fn set_proxy_config() {
    let mut deps = init_with_registered_vault();

    execute(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        HubExecuteMsg::from(HubAdminMsg::SetProxyConfig {
            vault: VAULT.into(),
            deposit: Some("deposit_proxy".into()),
            advance: Some("advance_proxy".into()),
            redeem: Some("redeem_proxy".into()),
            mint: Some("mint_proxy".into()),
        })
        .into(),
    )
    .unwrap();

    let vault_metadata = query(
        deps.as_ref(),
        mock_env(),
        HubQueryMsg::VaultMetadata {
            vault: VAULT.into(),
        }
        .into(),
    )
    .map(into_response::<VaultMetadata>)
    .unwrap();

    check(
        (
            vault_metadata.deposit_proxy,
            vault_metadata.advance_proxy,
            vault_metadata.mint_proxy,
            vault_metadata.redeem_proxy,
        ),
        expect![[
            r#"(Some("deposit_proxy"), Some("advance_proxy"), Some("mint_proxy"), Some("redeem_proxy"))"#
        ]],
    )
}

#[test]
fn admin() {
    let mut deps = init_with_registered_vault();

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
