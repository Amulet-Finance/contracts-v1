use cosmwasm_std::{
    coins, from_json,
    testing::{mock_dependencies, mock_env, MockQuerier},
    to_json_binary, Addr, Binary, ContractResult, DepsMut, MessageInfo, Querier, QuerierResult,
    QuerierWrapper, QueryRequest, SystemError, SystemResult,
};
use neutron_sdk::bindings::query::NeutronQuery;

use amulet_ntrn::query::{
    IcqParams, InterchainTxsParams, QueryIcqParamsResponse, QueryInterchainTxParamsResponse,
};

use test_utils::{check, prelude::expect};

use crate::{execute, instantiate, msg::Config, InstantiateMsg};

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

#[derive(Default)]
struct QueryWrapper(MockQuerier);

impl Querier for QueryWrapper {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<NeutronQuery> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {e}"),
                    request: bin_request.into(),
                })
            }
        };

        let res: ContractResult<Binary> = match request {
            QueryRequest::Stargate { path, .. } => match path.as_str() {
                InterchainTxsParams::QUERY_PATH => {
                    to_json_binary(&QueryInterchainTxParamsResponse {
                        params: InterchainTxsParams {
                            msg_submit_tx_max_messages: 16u64.into(),
                            register_fee: coins(100_000, "untrn"),
                        },
                    })
                    .into()
                }
                IcqParams::QUERY_PATH => to_json_binary(&QueryIcqParamsResponse {
                    params: IcqParams {
                        query_submit_timeout: "1036800".to_owned(),
                        query_deposit: coins(1_000_000, "untrn"),
                        tx_query_removal_limit: "10000".to_owned(),
                    },
                })
                .into(),
                _ => {
                    return SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: path.to_string(),
                    })
                }
            },
            _ => return self.0.raw_query(bin_request),
        };

        SystemResult::Ok(res)
    }
}

fn config() -> Config {
    Config {
        connection_id: "connection-0".to_owned(),
        estimated_block_interval_seconds: 3,
        fee_bps_block_increment: 1,
        fee_payment_cooldown_blocks: 28800,
        icq_update_interval: 10_000,
        interchain_tx_timeout_seconds: 60 * 60,
        max_fee_bps: 200,
        max_unbonding_entries: 7,
        max_validators_per_delegations_icq: 15,
        remote_denom: "stake".to_owned(),
        remote_denom_decimals: 6,
        transfer_in_channel: "channel-0".to_owned(),
        transfer_in_timeout_seconds: 60 * 60,
        transfer_out_channel: "channel-1".to_owned(),
        transfer_out_timeout_seconds: 60 * 60,
        unbonding_period: 21 * 24 * 60 * 60,
    }
}

#[test]
fn instantiate_non_unique_validator_set_fails() {
    let mut deps = mock_dependencies();

    let err = instantiate(
        deps.as_mut(),
        mock_env(),
        info!("creator"),
        InstantiateMsg {
            config: config(),
            initial_validator_set: vec![
                "val1".to_owned(),
                "val2".to_owned(),
                "val2".to_owned(),
                "val3".to_owned(),
            ],
            initial_validator_weights: vec![2500, 2500, 2500, 2500],
        },
    )
    .unwrap_err();

    check(
        err.to_string(),
        expect![[r#""validator val2 occurs more than once in the validator set""#]],
    )
}

#[test]
fn redelegate_to_existing_validator_fails() {
    let mut deps = mock_dependencies();

    instantiate(
        DepsMut {
            storage: &mut deps.storage,
            api: &deps.api,
            querier: QuerierWrapper::new(&QueryWrapper::default()),
        },
        mock_env(),
        info!("creator", 3_200_000, "untrn"),
        InstantiateMsg {
            config: config(),
            initial_validator_set: vec![
                "val1".to_owned(),
                "val2".to_owned(),
                "val3".to_owned(),
                "val4".to_owned(),
            ],
            initial_validator_weights: vec![2500, 2500, 2500, 2500],
        },
    )
    .unwrap();

    let err = execute(
        DepsMut {
            storage: &mut deps.storage,
            api: &deps.api,
            querier: QuerierWrapper::new(&QueryWrapper::default()),
        },
        mock_env(),
        info!("creator", 1_000_000, "untrn"),
        crate::msg::ExecuteMsg::Strategy(crate::msg::StrategyExecuteMsg::RedelegateSlot {
            slot: 0,
            validator: "val4".to_owned(),
        }),
    )
    .unwrap_err();

    check(
        err.to_string(),
        expect![[r#""val4 already exists in the set""#]],
    )
}
