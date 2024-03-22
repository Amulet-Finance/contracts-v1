use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CustomQuery, QuerierWrapper};

use amulet_core::{
    hub::{AdvanceFee, AdvanceFeeOracle as CoreAdvanceFeeOracle, Oracle},
    Recipient,
};

pub struct AdvanceFeeOracle<'a> {
    querier: QuerierWrapper<'a>,
}

#[cw_serde]
pub struct AdvanceFeeQuery {
    recipient: String,
}

#[cw_serde]
pub struct AdvanceFeeResponse {
    fee: Option<u32>,
}

impl<'a> AdvanceFeeOracle<'a> {
    pub fn new(querier: QuerierWrapper<'a, impl CustomQuery>) -> Self {
        Self {
            querier: querier.into_empty(),
        }
    }
}

impl<'a> CoreAdvanceFeeOracle for AdvanceFeeOracle<'a> {
    fn advance_fee(&self, oracle: &Oracle, recipient: &Recipient) -> Option<AdvanceFee> {
        let response: AdvanceFeeResponse = match self.querier.query_wasm_smart(
            oracle,
            &AdvanceFeeQuery {
                recipient: recipient.into(),
            },
        ) {
            Ok(res) => res,
            Err(err) => panic!("unexpected error querying advance fee oracle: {err}"),
        };

        response.fee.and_then(AdvanceFee::new)
    }
}
