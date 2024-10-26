use amulet_cw::mint::TokenFactory as _;
use amulet_ntrn::token_factory::TokenFactory;
use cosmwasm_std::{CosmosMsg, Deps, Env, Storage};

use amulet_core::{
    mint::Ticker,
    vault::{
        ClaimAmount, DepositAmount, DepositValue, Now, Strategy as CoreStrategy, StrategyCmd,
        TotalDepositsValue, UnbondEpoch, UnbondReadyStatus,
    },
    Asset, Decimals,
};
use neutron_sdk::bindings::msg::NeutronMsg;

use crate::{cw20, state::StorageExt as _, PLACEHOLDER_TOKEN_TICKER};

pub struct Strategy<'a> {
    deps: Deps<'a>,
    env: &'a Env,
}

impl<'a> Strategy<'a> {
    pub fn new(deps: Deps<'a>, env: &'a Env) -> Self {
        Self { deps, env }
    }
}

impl<'a> CoreStrategy for Strategy<'a> {
    fn now(&self) -> Now {
        self.env.block.time.seconds()
    }

    fn deposit_asset(&self) -> Asset {
        TokenFactory::new(self.env)
            .denom(&Ticker::new(PLACEHOLDER_TOKEN_TICKER))
            .into()
    }

    fn underlying_asset_decimals(&self) -> Decimals {
        self.deps.storage.underlying_decimals()
    }

    fn total_deposits_value(&self) -> TotalDepositsValue {
        TotalDepositsValue(self.deps.storage.deposits())
    }

    fn deposit_value(&self, DepositAmount(amount): DepositAmount) -> DepositValue {
        DepositValue(amount)
    }

    fn unbond(&self, DepositValue(deposit_value): DepositValue) -> UnbondReadyStatus {
        UnbondReadyStatus::Ready {
            amount: ClaimAmount(deposit_value),
            epoch: UnbondEpoch {
                start: self.env.block.time.seconds(),
                end: self.env.block.time.seconds(),
            },
        }
    }
}

fn increase_deposits(storage: &mut dyn Storage, amount: u128) {
    let deposits = storage
        .deposits()
        .checked_add(amount)
        .expect("deposit balance should never overflow");

    storage.set_deposits(deposits);
}

fn decrease_deposits(storage: &mut dyn Storage, amount: u128) {
    let deposits = storage
        .deposits()
        .checked_sub(amount)
        .expect("decrease amount should always be <= balance");

    storage.set_deposits(deposits);
}

pub fn handle_cmd(
    storage: &mut dyn Storage,
    env: &Env,
    cmd: StrategyCmd,
) -> Option<CosmosMsg<NeutronMsg>> {
    match cmd {
        StrategyCmd::Deposit {
            amount: DepositAmount(amount),
        } => {
            let token_factory = TokenFactory::new(env);

            let burn_placeholder_msg = token_factory.burn(
                token_factory
                    .denom(&Ticker::new(PLACEHOLDER_TOKEN_TICKER))
                    .into(),
                amount,
            );

            increase_deposits(storage, amount);

            Some(burn_placeholder_msg)
        }

        StrategyCmd::Unbond {
            value: DepositValue(amount),
        } => {
            decrease_deposits(storage, amount);
            None
        }

        StrategyCmd::SendClaimed {
            amount: ClaimAmount(amount),
            recipient,
        } => {
            let cw20 = storage.cw20();

            let transfer_msg = cw20::transfer(amount, recipient.as_str(), &cw20);

            Some(transfer_msg)
        }
    }
}
