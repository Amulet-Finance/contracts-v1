pub mod msg;

use anyhow::Error;
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    Addr, StdError
};
use amulet_cw::strategy::generic_lst::RedemptionRateResponse;
use msg::{InstantiateMsg, QueryMsg, ExchangeRateQuery};
use cw_storage_plus::Item;
use thiserror::Error;

pub const CORE_CONTRACT: Item<Addr> = Item::new("core_contract");

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] cosmwasm_std::StdError),

    #[error("Invalid core contract address")]
    InvalidCoreContractAddress {},
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let core_contract = deps.api.addr_validate(&msg.core_contract)
        .map_err(|_| ContractError::InvalidCoreContractAddress {})?;

    CORE_CONTRACT.save(deps.storage, &core_contract)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("core_contract", msg.core_contract))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::RedemptionRate {} => {
            let core_contract = CORE_CONTRACT.load(deps.storage)
                .map_err(|err| StdError::generic_err(format!("Failed to load core contract: {}", err)))?;

            let drop_query = ExchangeRateQuery::ExchangeRate {};

            let res: Decimal = deps.querier.query_wasm_smart(&core_contract, &drop_query)
                .map_err(|err| StdError::generic_err(format!("Query to core contract failed: {}", err)))?;

            let response = RedemptionRateResponse {
                rate: res
            };

            to_json_binary(&response)?
        },
    };

    Ok(binary)
}


#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{from_json, to_json_binary, Querier, OwnedDeps};
    use cosmwasm_std::testing::{mock_env, mock_info, MockQuerier, MockStorage, MockApi};
    use cosmwasm_std::{SystemResult, ContractResult, QueryRequest, WasmQuery};
    use core::str::FromStr;

    struct MockQuerierWrapper {
        base: MockQuerier,
    }

    impl MockQuerierWrapper {
        pub fn new(base: MockQuerier) -> Self {
            MockQuerierWrapper { base }
        }

        fn handle_wasm_query(&self, request: &QueryRequest<Binary>) -> SystemResult<ContractResult<Binary>> {
            if let QueryRequest::Wasm(WasmQuery::Smart { contract_addr, .. }) = request {
                if contract_addr == "neutron1elxhch2kul3qk2whxawtfwe0l2ma0snec3fe6j4zp2wftwrhs33q2yzqwy" {
                    let res = Decimal::from_str("1.0").unwrap();
                    return SystemResult::Ok(ContractResult::Ok(to_json_binary(&res).unwrap()));
                }
            }
            self.base.handle_query(&convert_query(request))
        }
    }

    impl Querier for MockQuerierWrapper {
        fn raw_query(&self, bin_request: &[u8]) -> SystemResult<ContractResult<Binary>> {
            let request: QueryRequest<Binary> = from_json(bin_request).unwrap();
            self.handle_wasm_query(&request)
        }
    }

    fn convert_query(request: &QueryRequest<Binary>) -> QueryRequest<cosmwasm_std::Empty> {
        serde_json::from_str(&serde_json::to_string(request).unwrap()).unwrap()
    }

    fn mock_dependencies_with_custom_querier() -> OwnedDeps<MockStorage, MockApi, MockQuerierWrapper> {
        let base = MockQuerier::new(&[]);
        OwnedDeps {
            storage: MockStorage::new(),
            api: MockApi::default(),
            querier: MockQuerierWrapper::new(base),
            custom_query_type: std::marker::PhantomData,
        }
    }

    #[test]
    fn test_redemption_rate_query() {
        let mut deps = mock_dependencies_with_custom_querier();

        let msg = InstantiateMsg {
            core_contract: "neutron1elxhch2kul3qk2whxawtfwe0l2ma0snec3fe6j4zp2wftwrhs33q2yzqwy".to_string(),
        };
        let _res = instantiate(deps.as_mut(), mock_env(), mock_info("owner", &[]), msg).unwrap();

        let resp = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::RedemptionRate {}
        ).unwrap();
        let resp: RedemptionRateResponse = from_json(&resp).unwrap();

        assert_eq!(
            resp,
            RedemptionRateResponse {
                rate: Decimal::from_str("1").unwrap()
            }
        );
    }
}
