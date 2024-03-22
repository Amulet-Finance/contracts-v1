pub mod generic_lst {
    use cosmwasm_schema::{cw_serde, QueryResponses};
    use cosmwasm_std::{CustomQuery, Decimal, QuerierWrapper, StdError};

    #[cw_serde]
    pub struct RedemptionRateResponse {
        pub rate: Decimal,
    }

    #[cw_serde]
    #[derive(QueryResponses)]
    pub enum QueryMsg {
        #[returns(RedemptionRateResponse)]
        RedemptionRate {},
    }

    pub trait QuerierExt {
        fn redemption_rate(&self, oracle: &str) -> Result<Decimal, StdError>;
    }

    impl<'a, C: CustomQuery> QuerierExt for QuerierWrapper<'a, C> {
        fn redemption_rate(&self, oracle: &str) -> Result<Decimal, StdError> {
            self.query_wasm_smart(oracle, &QueryMsg::RedemptionRate {})
                .map(|response: RedemptionRateResponse| response.rate)
        }
    }
}
