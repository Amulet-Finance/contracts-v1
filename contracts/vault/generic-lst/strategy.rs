use core::panic;

use amulet_cw::strategy::generic_lst::QuerierExt;
use anyhow::Error;
use cosmwasm_std::{
    coins, BankMsg, CosmosMsg, CustomQuery, Decimal, Env, Fraction, QuerierWrapper, Storage,
    Timestamp, Uint128,
};

use amulet_core::{
    vault::{
        DepositAmount, DepositValue, Now, Strategy as CoreStrategy, StrategyCmd,
        TotalDepositsValue, UnbondEpoch, UnbondReadyStatus,
    },
    Asset, Decimals,
};

use crate::state::StorageExt as _;

#[derive(Debug, Clone, Copy)]
pub struct LstRedemptionRate(Decimal);

fn apply_rate(rate: Decimal, amount: u128) -> u128 {
    let Ok(result) = Uint128::new(amount)
        .checked_multiply_ratio(rate.atomics(), 10u128.pow(Decimal::DECIMAL_PLACES))
    else {
        panic!("applying rate {rate} to {amount} resulted in an overflow");
    };

    result.u128()
}

impl LstRedemptionRate {
    fn lst_to_underlying(self, amount: DepositAmount) -> u128 {
        apply_rate(self.0, amount)
    }

    fn underlying_to_lst(self, value: DepositValue) -> u128 {
        let inverse_rate = self
            .0
            .inv()
            .expect("redemption rate inverse should never be NaN");

        apply_rate(inverse_rate, value)
    }
}

pub fn lst_redeption_rate(
    storage: &dyn Storage,
    querier: QuerierWrapper<impl CustomQuery>,
) -> Result<LstRedemptionRate, Error> {
    let oracle = storage.lst_redemption_rate_oracle();

    let redemption_rate = querier.redemption_rate(&oracle)?;

    Ok(LstRedemptionRate(redemption_rate))
}

pub struct Strategy<'a> {
    storage: &'a dyn Storage,
    now: Timestamp,
    redemption_rate: LstRedemptionRate,
}

impl<'a> Strategy<'a> {
    pub fn new(storage: &'a dyn Storage, env: &Env, redemption_rate: LstRedemptionRate) -> Self {
        Self {
            storage,
            now: env.block.time,
            redemption_rate,
        }
    }
}

impl<'a> CoreStrategy for Strategy<'a> {
    fn now(&self) -> Now {
        self.now.seconds()
    }

    fn deposit_asset(&self) -> Asset {
        self.storage.lst_denom().into()
    }

    fn underlying_asset_decimals(&self) -> Decimals {
        self.storage.underlying_decimals()
    }

    fn total_deposits_value(&self) -> TotalDepositsValue {
        let active_lst_balance = self.storage.active_lst_balance();

        self.redemption_rate.lst_to_underlying(active_lst_balance)
    }

    fn deposit_value(&self, amount: DepositAmount) -> DepositValue {
        self.redemption_rate.lst_to_underlying(amount)
    }

    fn unbond(&self, value: DepositValue) -> UnbondReadyStatus {
        let amount = self.redemption_rate.underlying_to_lst(value);

        UnbondReadyStatus::Ready {
            amount,
            epoch: UnbondEpoch {
                start: self.now.seconds(),
                end: self.now.seconds(),
            },
        }
    }
}

fn increase_active_deposits(storage: &mut dyn Storage, amount: DepositAmount) {
    let active_lst_balance = storage
        .active_lst_balance()
        .checked_add(amount)
        .expect("active lst balance should never overflow");

    storage.set_active_lst_balance(active_lst_balance);
}

fn decrease_active_deposits(storage: &mut dyn Storage, amount: DepositAmount) {
    let active_lst_balance = storage
        .active_lst_balance()
        .checked_sub(amount)
        .expect("decrease amount should always be <= active balance");

    storage.set_active_lst_balance(active_lst_balance);
}

fn increase_claimable_deposits(storage: &mut dyn Storage, amount: DepositAmount) {
    let claimable_lst_balance = storage
        .claimable_lst_balance()
        .checked_add(amount)
        .expect("claimable lst balance should never overflow");

    storage.set_claimable_lst_balance(claimable_lst_balance);
}

fn decrease_claimable_deposits(storage: &mut dyn Storage, amount: DepositAmount) {
    let claimable_lst_balance = storage
        .claimable_lst_balance()
        .checked_sub(amount)
        .expect("decrease amount should always be <= claimable balance");

    storage.set_claimable_lst_balance(claimable_lst_balance);
}

pub fn handle_cmd<CustomMsg>(
    storage: &mut dyn Storage,
    redemption_rate: LstRedemptionRate,
    cmd: StrategyCmd,
) -> Option<CosmosMsg<CustomMsg>> {
    match cmd {
        StrategyCmd::Deposit { amount } => {
            increase_active_deposits(storage, amount);

            None
        }

        StrategyCmd::Unbond { value } => {
            let lst_amount = redemption_rate.underlying_to_lst(value);

            decrease_active_deposits(storage, lst_amount);

            increase_claimable_deposits(storage, lst_amount);

            None
        }

        StrategyCmd::SendClaimed { amount, recipient } => {
            decrease_claimable_deposits(storage, amount);

            let lst_denom = storage.lst_denom();

            Some(
                BankMsg::Send {
                    to_address: recipient.into_string(),
                    amount: coins(amount, lst_denom),
                }
                .into(),
            )
        }
    }
}
