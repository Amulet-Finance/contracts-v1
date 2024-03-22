use amulet_core::{
    mint::{Synthetic, SyntheticAmount, Ticker},
    Decimals, Recipient,
};
use amulet_cw::mint::TokenFactory as CwTokenFactory;
use cosmwasm_std::{Env, SubMsg};
use neutron_sdk::bindings::msg::NeutronMsg;

pub struct TokenFactory<'a>(&'a Env);

impl<'a> TokenFactory<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self(env)
    }
}

impl<'a> CwTokenFactory<NeutronMsg> for TokenFactory<'a> {
    fn denom(&self, ticker: &Ticker) -> String {
        format!("factory/{}/{ticker}", self.0.contract.address)
    }

    fn create(&self, ticker: Ticker, _: Decimals) -> SubMsg<NeutronMsg> {
        SubMsg::new(NeutronMsg::submit_create_denom(ticker))
    }

    fn mint(
        &self,
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> SubMsg<NeutronMsg> {
        SubMsg::new(NeutronMsg::submit_mint_tokens(
            synthetic,
            amount.into(),
            recipient,
        ))
    }

    fn burn(&self, synthetic: Synthetic, amount: SyntheticAmount) -> SubMsg<NeutronMsg> {
        SubMsg::new(NeutronMsg::submit_burn_tokens(synthetic, amount.into()))
    }
}
