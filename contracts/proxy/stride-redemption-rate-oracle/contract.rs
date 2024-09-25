use amulet_cw::{
    strategy::generic_lst::{QueryMsg, RedemptionRateResponse},
    StorageExt as _,
};
use anyhow::Result;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
};

const STRIDE_ICA_ORACLE_CONTRACT_KEY: &str = "stride_ica_oracle";
const STRIDE_ASSET_DENOM_KEY: &str = "stride_asset_denom";

#[cw_serde]
pub struct InstantiateMsg {
    pub stride_ica_oracle: String,
    pub stride_asset_denom: String,
}

#[cw_serde]
pub struct StrideRedemptionRateResponse {
    pub redemption_rate: Decimal,
    pub update_time: u64,
}

#[cw_serde]
pub enum StrideOracleQuery {
    RedemptionRate { denom: String },
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    let stride_ica_oracle = deps.api.addr_validate(&msg.stride_ica_oracle)?;

    deps.storage
        .set_string(STRIDE_ICA_ORACLE_CONTRACT_KEY, stride_ica_oracle.as_str());

    deps.storage
        .set_string(STRIDE_ASSET_DENOM_KEY, &msg.stride_asset_denom);

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("stride_ica_oracle", msg.stride_ica_oracle)
        .add_attribute("stride_asset_denom", msg.stride_asset_denom))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary> {
    let binary = match msg {
        QueryMsg::RedemptionRate {} => {
            let oracle = deps
                .storage
                .string_at(STRIDE_ICA_ORACLE_CONTRACT_KEY)
                .expect("set during initialization");

            let denom = deps
                .storage
                .string_at(STRIDE_ASSET_DENOM_KEY)
                .expect("set during initialization");

            let stride_oracle_query = StrideOracleQuery::RedemptionRate { denom };

            let res: StrideRedemptionRateResponse = deps
                .querier
                .query_wasm_smart(oracle, &stride_oracle_query)?;

            let response = RedemptionRateResponse {
                rate: res.redemption_rate,
            };

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

    const MOCK_ICA_ORACLE_ADDRESS: &str =
        "neutron1elxhch2kul3qk2whxawtfwe0l2ma0snec3fe6j4zp2wftwrhs33q2yzqwy";

    const MOCK_DENOM: &str = "stuatom";

    #[test]
    fn test_redemption_rate_query() {
        let mut deps = mock_dependencies();

        deps.querier.update_wasm(move |query| {
            let WasmQuery::Smart { msg, contract_addr } = query else {
                panic!("unexpected wasm query: {query:?}");
            };

            let binary = match contract_addr.as_str() {
                MOCK_ICA_ORACLE_ADDRESS => {
                    let StrideOracleQuery::RedemptionRate { denom }: StrideOracleQuery =
                        from_json(msg).unwrap();

                    assert_eq!(denom, MOCK_DENOM);

                    let redemption_rate: Decimal = "1.0".parse().unwrap();

                    to_json_binary(&StrideRedemptionRateResponse {
                        redemption_rate,
                        update_time: 1,
                    })
                }
                _ => panic!("unexpected contract query addr: {contract_addr}"),
            }
            .unwrap();

            SystemResult::Ok(ContractResult::Ok(binary))
        });

        let msg = InstantiateMsg {
            stride_ica_oracle: MOCK_ICA_ORACLE_ADDRESS.to_owned(),
            stride_asset_denom: MOCK_DENOM.to_owned(),
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
