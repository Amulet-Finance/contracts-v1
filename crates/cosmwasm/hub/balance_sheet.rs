use cosmwasm_std::{coins, BankMsg, Env, Storage, SubMsg};

use amulet_core::{
    hub::{
        Account, AmoShares, BalanceSheet as CoreBalanceSheet, BalanceSheetCmd, Collateral, Credit,
        Debt, SumPaymentRatio, Treasury, TreasuryShares, VaultId,
    },
    vault::SharesAmount,
};

use crate::StorageExt as _;

pub struct BalanceSheet<'a>(&'a dyn Storage);

impl<'a> BalanceSheet<'a> {
    pub fn new(storage: &'a dyn Storage) -> Self {
        Self(storage)
    }
}

#[rustfmt::skip]
mod key {
    use crate::MapKey;

    macro_rules! key {
        ($k:literal) => {
            concat!("hub_balance_sheet::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            crate::MapKey::new(key!($k))
        };
    }

    pub const TREASURY                  : &str   = key!("treasury");
    pub const COLLATERAL_SHARES         : MapKey = map_key!("collateral_shares");
    pub const COLLATERAL_BALANCE        : MapKey = map_key!("collateral_balance");
    pub const RESERVE_SHARES            : MapKey = map_key!("reserve_shares");
    pub const RESERVE_BALANCE           : MapKey = map_key!("reserve_balance");
    pub const TREASURY_SHARES           : MapKey = map_key!("treasury_shares");
    pub const AMO_SHARES                : MapKey = map_key!("amo_shares");
    pub const OVERALL_SUM_PAYMENT_RATIO : MapKey = map_key!("overall_sum_payment_ratio");
    pub const ACCOUNT_COLLATERAL        : MapKey = map_key!("account_collateral");
    pub const ACCOUNT_DEBT              : MapKey = map_key!("account_debt");
    pub const ACCOUNT_CREDIT            : MapKey = map_key!("account_credit");
    pub const ACCOUNT_SUM_PAYMENT_RATIO : MapKey = map_key!("account_sum_payment_ratio");
}

const TIMESTAMP: &str = "timestamp";

pub trait StorageExt: Storage {
    fn set_overall_spr_timestamp(&mut self, vault: &str, timestamp: u64) {
        self.set_u64(
            key::OVERALL_SUM_PAYMENT_RATIO.multi([&vault, &TIMESTAMP]),
            timestamp,
        );
    }

    fn overall_spr_timestamp(&self, vault: &str) -> Option<u64> {
        self.u64_at(key::OVERALL_SUM_PAYMENT_RATIO.multi([&vault, &TIMESTAMP]))
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

impl<'a> CoreBalanceSheet for BalanceSheet<'a> {
    fn treasury(&self) -> Option<Treasury> {
        self.0.string_at(key::TREASURY).map(Into::into)
    }

    fn collateral_shares(&self, vault: &VaultId) -> Option<SharesAmount> {
        self.0.u128_at(key::COLLATERAL_SHARES.with(vault))
    }

    fn collateral_balance(&self, vault: &VaultId) -> Option<Collateral> {
        self.0.u128_at(key::COLLATERAL_BALANCE.with(vault))
    }

    fn reserve_shares(&self, vault: &VaultId) -> Option<SharesAmount> {
        self.0.u128_at(key::RESERVE_SHARES.with(vault))
    }

    fn reserve_balance(&self, vault: &VaultId) -> Option<Collateral> {
        self.0.u128_at(key::RESERVE_BALANCE.with(vault))
    }

    fn treasury_shares(&self, vault: &VaultId) -> Option<TreasuryShares> {
        self.0.u128_at(key::TREASURY_SHARES.with(vault))
    }

    fn amo_shares(&self, vault: &VaultId) -> Option<AmoShares> {
        self.0.u128_at(key::AMO_SHARES.with(vault))
    }

    fn overall_sum_payment_ratio(&self, vault: &VaultId) -> Option<SumPaymentRatio> {
        self.0
            .u256_at(key::OVERALL_SUM_PAYMENT_RATIO.with(vault))
            .map(SumPaymentRatio::raw)
    }

    fn account_collateral(&self, vault: &VaultId, account: &Account) -> Option<Collateral> {
        self.0
            .u128_at(key::ACCOUNT_COLLATERAL.multi([vault, account]))
    }

    fn account_debt(&self, vault: &VaultId, account: &Account) -> Option<Debt> {
        self.0.u128_at(key::ACCOUNT_DEBT.multi([vault, account]))
    }

    fn account_credit(&self, vault: &VaultId, account: &Account) -> Option<Credit> {
        self.0.u128_at(key::ACCOUNT_CREDIT.multi([vault, account]))
    }

    fn account_sum_payment_ratio(
        &self,
        vault: &VaultId,
        account: &Account,
    ) -> Option<SumPaymentRatio> {
        self.0
            .u256_at(key::ACCOUNT_SUM_PAYMENT_RATIO.multi([vault, account]))
            .map(SumPaymentRatio::raw)
    }
}

pub fn handle_cmd<Msg>(
    storage: &mut dyn Storage,
    env: &Env,
    cmd: BalanceSheetCmd,
) -> Option<SubMsg<Msg>> {
    match cmd {
        BalanceSheetCmd::SetTreasury { treasury } => storage.set_string(key::TREASURY, &treasury),

        BalanceSheetCmd::SetCollateralShares { vault, shares } => {
            storage.set_u128(key::COLLATERAL_SHARES.with(vault), shares)
        }

        BalanceSheetCmd::SetCollateralBalance { vault, balance } => {
            storage.set_u128(key::COLLATERAL_BALANCE.with(vault), balance)
        }

        BalanceSheetCmd::SetReserveShares { vault, shares } => {
            storage.set_u128(key::RESERVE_SHARES.with(vault), shares)
        }

        BalanceSheetCmd::SetReserveBalance { vault, balance } => {
            storage.set_u128(key::RESERVE_BALANCE.with(vault), balance)
        }

        BalanceSheetCmd::SetTreasuryShares { vault, shares } => {
            storage.set_u128(key::TREASURY_SHARES.with(vault), shares)
        }

        BalanceSheetCmd::SetAmoShares { vault, shares } => {
            storage.set_u128(key::AMO_SHARES.with(vault), shares)
        }

        BalanceSheetCmd::SetOverallSumPaymentRatio { vault, spr } => {
            storage.set_u256(key::OVERALL_SUM_PAYMENT_RATIO.with(&vault), spr.into_raw());
            storage.set_overall_spr_timestamp(&vault, env.block.time.seconds());
        }

        BalanceSheetCmd::SetAccountCollateral {
            vault,
            account,
            collateral,
        } => storage.set_u128(
            key::ACCOUNT_COLLATERAL.multi([&vault, &account]),
            collateral,
        ),

        BalanceSheetCmd::SetAccountDebt {
            vault,
            account,
            debt,
        } => storage.set_u128(key::ACCOUNT_DEBT.multi([&vault, &account]), debt),

        BalanceSheetCmd::SetAccountCredit {
            vault,
            account,
            credit,
        } => storage.set_u128(key::ACCOUNT_CREDIT.multi([&vault, &account]), credit),

        BalanceSheetCmd::SetAccountSumPaymentRatio {
            vault,
            account,
            spr,
        } => storage.set_u256(
            key::ACCOUNT_SUM_PAYMENT_RATIO.multi([&vault, &account]),
            spr.into_raw(),
        ),

        BalanceSheetCmd::SendShares {
            shares,
            amount,
            recipient,
        } => {
            return Some(SubMsg::new(BankMsg::Send {
                to_address: recipient.into_string(),
                amount: coins(amount, shares),
            }))
        }
    }

    None
}
