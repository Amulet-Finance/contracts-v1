use amulet_cw::{
    strategy::generic_lst::{QueryMsg, RedemptionRateResponse},
    StorageExt as _,
};
use anyhow::Result;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
};

const DROP_CONTRACT_KEY: &str = "drop_contract";

#[cw_serde]
pub struct InstantiateMsg {
    pub drop_contract: String,
}

#[cw_serde]
pub enum ExchangeRateQuery {
    ExchangeRate {},
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    let drop_contract = deps.api.addr_validate(&msg.drop_contract)?;

    deps.storage
        .set_string(DROP_CONTRACT_KEY, drop_contract.as_str());

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("drop_contract", msg.drop_contract))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary> {
    let binary = match msg {
        QueryMsg::RedemptionRate {} => {
            let core_contract = deps
                .storage
                .string_at(DROP_CONTRACT_KEY)
                .expect("set during initialization");

            let drop_query = ExchangeRateQuery::ExchangeRate {};

            let res: Decimal = deps.querier.query_wasm_smart(core_contract, &drop_query)?;

            let response = RedemptionRateResponse { rate: res };

            to_json_binary(&response)?
        }
    };

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::{
        from_json,
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, ContractResult, SystemResult, WasmQuery,
    };

    const MOCK_DROP_CONTRACT_ADDRESS: &str =
        "neutron1elxhch2kul3qk2whxawtfwe0l2ma0snec3fe6j4zp2wftwrhs33q2yzqwy";

    #[test]
    fn test_redemption_rate_query() {
        let mut deps = mock_dependencies();

        deps.querier.update_wasm(move |query| {
            let WasmQuery::Smart { msg, contract_addr } = query else {
                panic!("unexpected wasm query: {query:?}");
            };

            let binary = match contract_addr.as_str() {
                MOCK_DROP_CONTRACT_ADDRESS => {
                    assert!(from_json::<ExchangeRateQuery>(msg).is_ok());
                    let res: Decimal = "1.0".parse().unwrap();
                    to_json_binary(&res)
                }
                _ => panic!("unexpected contract query addr: {contract_addr}"),
            }
            .unwrap();

            SystemResult::Ok(ContractResult::Ok(binary))
        });

        let msg = InstantiateMsg {
            drop_contract: MOCK_DROP_CONTRACT_ADDRESS.to_owned(),
        };

        instantiate(deps.as_mut(), mock_env(), mock_info("owner", &[]), msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::RedemptionRate {}).unwrap();

        assert_eq!(
            from_json(res),
            Ok(RedemptionRateResponse {
                rate: "1".parse().unwrap()
            })
        );
    }
}
