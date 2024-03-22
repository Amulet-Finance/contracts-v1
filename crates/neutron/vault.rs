pub mod mint {
    use cosmwasm_std::{CosmosMsg, Env, Storage};
    use neutron_sdk::bindings::msg::NeutronMsg;

    use amulet_core::{
        vault::{Mint as CoreMint, MintCmd, SharesAmount, TotalSharesIssued},
        Asset, Recipient,
    };
    use amulet_cw::StorageExt as _;

    pub struct Mint<'a> {
        storage: &'a dyn Storage,
        contract_address: &'a str,
    }

    pub const SHARES_DENOM: &str = "share";

    pub fn init_msg() -> CosmosMsg<NeutronMsg> {
        NeutronMsg::CreateDenom {
            subdenom: SHARES_DENOM.to_owned(),
        }
        .into()
    }

    #[rustfmt::skip]
    mod key {
        macro_rules! key {
            ($k:literal) => {
                concat!("vault_shares_mint::", $k)
            };
        }

        pub const TOTAL_ISSUED_SHARES: &str = key!("total_issued_shares");
    }

    impl<'a> Mint<'a> {
        pub fn new(storage: &'a dyn Storage, env: &'a Env) -> Self {
            Self {
                storage,
                contract_address: env.contract.address.as_str(),
            }
        }
    }

    fn total_shares_issued(storage: &dyn Storage) -> u128 {
        storage
            .u128_at(key::TOTAL_ISSUED_SHARES)
            .unwrap_or_default()
    }

    impl<'a> CoreMint for Mint<'a> {
        fn total_shares_issued(&self) -> TotalSharesIssued {
            total_shares_issued(self.storage)
        }

        fn shares_asset(&self) -> Asset {
            format!("factory/{}/{SHARES_DENOM}", self.contract_address).into()
        }
    }

    fn increase_total_shares_issued(storage: &mut dyn Storage, amount: SharesAmount) {
        let total_issued_shares = total_shares_issued(storage)
            .checked_add(amount)
            .expect("total issued shares balance should never overflow");

        storage.set_u128(key::TOTAL_ISSUED_SHARES, total_issued_shares);
    }

    fn decrease_total_shares_issued(storage: &mut dyn Storage, amount: SharesAmount) {
        let total_issued_shares = total_shares_issued(storage)
            .checked_sub(amount)
            .expect("amount <= total supply");

        storage.set_u128(key::TOTAL_ISSUED_SHARES, total_issued_shares);
    }

    fn mint_shares(amount: SharesAmount, recipient: Recipient) -> CosmosMsg<NeutronMsg> {
        NeutronMsg::MintTokens {
            denom: SHARES_DENOM.to_owned(),
            amount: amount.into(),
            mint_to_address: recipient.into_string(),
        }
        .into()
    }

    fn burn_shares(
        contract_address: impl Into<String>,
        amount: SharesAmount,
    ) -> CosmosMsg<NeutronMsg> {
        NeutronMsg::BurnTokens {
            denom: SHARES_DENOM.to_owned(),
            amount: amount.into(),
            burn_from_address: contract_address.into(),
        }
        .into()
    }

    pub fn handle_cmd(storage: &mut dyn Storage, env: &Env, cmd: MintCmd) -> CosmosMsg<NeutronMsg> {
        match cmd {
            MintCmd::Mint { amount, recipient } => {
                increase_total_shares_issued(storage, amount);
                mint_shares(amount, recipient)
            }

            MintCmd::Burn { amount } => {
                decrease_total_shares_issued(storage, amount);
                burn_shares(env.contract.address.clone(), amount)
            }
        }
    }
}

pub use mint::Mint;
