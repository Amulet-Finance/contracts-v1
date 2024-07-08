use neutron_sdk::bindings::msg::IbcFee;

pub mod token_factory;

pub mod query {
    use cosmwasm_schema::cw_serde;
    use cosmwasm_std::{Binary, Coin, CustomQuery, QuerierWrapper, QueryRequest, StdError, Uint64};

    #[cw_serde]
    pub struct InterchainTxsParams {
        pub msg_submit_tx_max_messages: Uint64,
        pub register_fee: Vec<Coin>,
    }

    impl InterchainTxsParams {
        pub const QUERY_PATH: &'static str = "/neutron.interchaintxs.v1.Query/Params";
    }

    #[cw_serde]
    pub struct QueryInterchainTxParamsResponse {
        pub params: InterchainTxsParams,
    }

    #[cw_serde]
    pub struct IcqParams {
        pub query_submit_timeout: String,
        pub query_deposit: Vec<Coin>,
        pub tx_query_removal_limit: String,
    }

    impl IcqParams {
        pub const QUERY_PATH: &'static str = "/neutron.interchainqueries.Query/Params";
    }

    #[cw_serde]
    pub struct QueryIcqParamsResponse {
        pub params: IcqParams,
    }

    pub trait QuerierExt {
        fn interchain_tx_max_msg_count(&self) -> Result<usize, StdError>;

        fn interchain_account_register_fee(&self) -> Result<Coin, StdError>;

        fn interchain_query_deposit(&self) -> Result<Coin, StdError>;
    }

    impl<'a, C: CustomQuery> QuerierExt for QuerierWrapper<'a, C> {
        fn interchain_tx_max_msg_count(&self) -> Result<usize, StdError> {
            let res: QueryInterchainTxParamsResponse = self.query(&QueryRequest::Stargate {
                path: InterchainTxsParams::QUERY_PATH.to_owned(),
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
                path: InterchainTxsParams::QUERY_PATH.to_owned(),
                data: Binary(vec![]),
            })?;

            let coin = res.params.register_fee.into_iter().next().unwrap();

            Ok(coin)
        }

        fn interchain_query_deposit(&self) -> Result<Coin, StdError> {
            let res: QueryIcqParamsResponse = self.query(&QueryRequest::Stargate {
                path: IcqParams::QUERY_PATH.to_owned(),
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
