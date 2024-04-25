use neutron_sdk::bindings::msg::IbcFee;

pub mod token_factory;

pub mod query {
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::{Binary, Coin, CustomQuery, QuerierWrapper, QueryRequest, StdError, Uint64};

    #[cw_serde]
    struct InterchainTxsParams {
        msg_submit_tx_max_messages: Uint64,
        register_fee: Vec<Coin>,
    }

    impl InterchainTxsParams {
        const TYPE_URL: &'static str = "/neutron.interchaintxs.v1.Query/Params";
    }

    #[cw_serde]
    struct QueryInterchainTxParamsResponse {
        params: InterchainTxsParams,
    }

    pub trait QuerierExt {
        fn interchain_tx_max_msg_count(&self) -> Result<usize, StdError>;

        fn interchain_account_register_fee(&self) -> Result<Coin, StdError>;

        fn interchain_query_deposit(&self) -> Result<Coin, StdError>;
    }

    impl<'a, C: CustomQuery> QuerierExt for QuerierWrapper<'a, C> {
        fn interchain_tx_max_msg_count(&self) -> Result<usize, StdError> {
            let res: QueryInterchainTxParamsResponse = self.query(&QueryRequest::Stargate {
                path: InterchainTxsParams::TYPE_URL.to_owned(),
                data: Binary(vec![]),
            })?;

            let max_msg_count = res
                .params
                .msg_submit_tx_max_messages
                .u64()
                .try_into()
                .expect("max msg count < usize::MAX");

            Ok(max_msg_count)
        }

        fn interchain_account_register_fee(&self) -> Result<Coin, StdError> {
            let res: QueryInterchainTxParamsResponse = self.query(&QueryRequest::Stargate {
                path: InterchainTxsParams::TYPE_URL.to_owned(),
                data: Binary(vec![]),
            })?;

            let coin = res.params.register_fee.into_iter().next().unwrap();

            Ok(coin)
        }

        fn interchain_query_deposit(&self) -> Result<Coin, StdError> {
            #[cw_serde]
            struct Params {
                query_submit_timeout: String,
                query_deposit: Vec<Coin>,
                tx_query_removal_limit: String,
            }

            #[cw_serde]
            struct QueryParamsResponse {
                params: Params,
            }

            let res: QueryParamsResponse = self.query(&QueryRequest::Stargate {
                path: "/neutron.interchainqueries.Query/Params".to_owned(),
                data: Binary(vec![]),
            })?;

            let coin = res.params.query_deposit.into_iter().next().unwrap();

            Ok(coin)
        }
    }
}

pub static IBC_FEE_DENOM: &str = "untrn";

pub trait IbcFeeExt {
    fn total_fee_per_tx(&self) -> u128;
}

impl IbcFeeExt for IbcFee {
    fn total_fee_per_tx(&self) -> u128 {
        self.timeout_fee
            .iter()
            .chain(self.ack_fee.iter())
            .filter_map(|c| (c.denom == IBC_FEE_DENOM).then_some(c.amount.u128()))
            .sum()
    }
}
