use amulet_core::{
    mint::{Synthetic, SyntheticAmount, Ticker},
    Decimals, Recipient,
};
use amulet_cw::mint::TokenFactory as CwTokenFactory;
use cosmwasm_std::{CosmosMsg, Env};
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

    fn create(&self, ticker: Ticker, _: Decimals) -> CosmosMsg<NeutronMsg> {
        NeutronMsg::submit_create_denom(ticker).into()
    }

    fn mint(
        &self,
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> CosmosMsg<NeutronMsg> {
        NeutronMsg::submit_mint_tokens(synthetic, amount.into(), recipient).into()
    }

    fn burn(&self, synthetic: Synthetic, amount: SyntheticAmount) -> CosmosMsg<NeutronMsg> {
        NeutronMsg::submit_burn_tokens(synthetic, amount.into()).into()
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::testing::mock_env;
    use test_utils::prelude::*;

    use super::*;

    fn ticker(s: &str) -> Ticker {
        s.to_owned().into()
    }

    #[test]
    fn denom() {
        check(
            TokenFactory::new(&mock_env()).denom(&ticker("amNTRN")),
            expect![[r#""factory/cosmos2contract/amntrn""#]],
        );
    }

    #[test]
    fn create() {
        check(
            TokenFactory::new(&mock_env()).create(ticker("amNTRN"), 6),
            expect![[r#"
                custom(create_denom(
                  subdenom: "amntrn",
                ))"#]],
        );
    }

    #[test]
    fn mint() {
        check(
            TokenFactory::new(&mock_env()).mint(
                "factory/cosmos2contract/amntrn".to_owned().into(),
                1_000_000,
                "bob".to_owned().into(),
            ),
            expect![[r#"
                custom(mint_tokens(
                  denom: "factory/cosmos2contract/amntrn",
                  amount: "1000000",
                  mint_to_address: "bob",
                ))"#]],
        );
    }

    #[test]
    fn burn() {
        check(
            TokenFactory::new(&mock_env()).burn(
                "factory/cosmos2contract/amntrn".to_owned().into(),
                1_000_000,
            ),
            expect![[r#"
                custom(burn_tokens(
                  denom: "factory/cosmos2contract/amntrn",
                  amount: "1000000",
                  burn_from_address: "",
                ))"#]],
        );
    }
}
