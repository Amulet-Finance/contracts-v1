use cosmwasm_std::{CosmosMsg, Env, Storage};

use amulet_core::{
    vault::{MintCmd, SharesAmount, SharesMint as CoreSharesMint, TotalSharesIssued},
    Asset,
};

use crate::{mint::TokenFactory, StorageExt as _};

pub struct SharesMint<'a> {
    storage: &'a dyn Storage,
    contract_address: &'a str,
}

pub const SHARES_DENOM: &str = "share";

pub fn init_msg<Msg>(factory: impl TokenFactory<Msg>) -> CosmosMsg<Msg> {
    factory.create(SHARES_DENOM.to_owned().into())
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

impl<'a> SharesMint<'a> {
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

impl<'a> CoreSharesMint for SharesMint<'a> {
    fn total_shares_issued(&self) -> TotalSharesIssued {
        TotalSharesIssued(total_shares_issued(self.storage))
    }

    fn shares_asset(&self) -> Asset {
        format!("factory/{}/{SHARES_DENOM}", self.contract_address).into()
    }
}

fn increase_total_shares_issued(storage: &mut dyn Storage, SharesAmount(amount): SharesAmount) {
    let total_issued_shares = total_shares_issued(storage)
        .checked_add(amount)
        .expect("total issued shares balance should never overflow");

    storage.set_u128(key::TOTAL_ISSUED_SHARES, total_issued_shares);
}

fn decrease_total_shares_issued(storage: &mut dyn Storage, SharesAmount(amount): SharesAmount) {
    let total_issued_shares = total_shares_issued(storage)
        .checked_sub(amount)
        .expect("amount <= total supply");

    storage.set_u128(key::TOTAL_ISSUED_SHARES, total_issued_shares);
}

pub fn handle_cmd<Msg>(
    storage: &mut dyn Storage,
    factory: impl TokenFactory<Msg>,
    cmd: MintCmd,
) -> CosmosMsg<Msg> {
    let full_denom = factory.denom(&SHARES_DENOM.to_owned().into()).into();

    match cmd {
        MintCmd::Mint { amount, recipient } => {
            increase_total_shares_issued(storage, amount);

            factory.mint(full_denom, amount.0, recipient)
        }

        MintCmd::Burn { amount } => {
            decrease_total_shares_issued(storage, amount);

            factory.burn(full_denom, amount.0)
        }
    }
}
