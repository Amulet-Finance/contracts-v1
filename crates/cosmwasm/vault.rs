pub mod mint;
pub mod unbonding_log;

use amulet_core::{
    vault::{
        offset_total_deposits_value, pending_batch_id, vault, ClaimableBatchIter, Cmd,
        DepositResponse as CoreDepositResponse, Error as CoreVaultError,
        SharesMint as CoreSharesMint, Strategy, UnbondEpoch, UnbondingLog as CoreUnbondingLog,
        Vault,
    },
    Decimals,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, Binary, Env, MessageInfo, Response, StdError, Storage, Uint128,
};

use crate::{non_payable, one_coin, PaymentError};

use self::unbonding_log::StorageExt as _;

pub use self::{
    mint::{handle_cmd as handle_mint_cmd, init_msg as init_mint_msg, SharesMint},
    unbonding_log::{handle_cmd as handle_unbonding_log_cmd, UnbondingLog},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Vault(#[from] CoreVaultError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    CosmWasm(#[from] StdError),
}

#[cw_serde]
pub struct DepositResponse {
    /// Total number of issued shares
    pub total_shares_issued: Uint128,
    /// The total of all deposits in terms the vault's underlying asset
    pub total_deposits_value: Uint128,
    /// Number of shares issued (minted) for deposit
    pub minted_shares: Uint128,
    /// The value of the deposit in terms the vault's underlying asset
    pub deposit_value: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Deposit native tokens into the vault, the sender receives the issued shares
    /// Responds with [DepositResponse]
    Deposit {},

    /// Donate native tokens to the vault
    Donate {},

    /// Allow one party to burn shares so that another party may claim the underlying deposits
    Redeem { recipient: String },

    /// Attempt to start any pending unbondings
    StartUnbond {},

    /// Claim any unclaimed unbonded underlying tokens belonging to the sender
    Claim {},
}

#[cw_serde]
pub struct UnbondingStatus {
    pub amount: Uint128,
    pub start: u64,
    pub end: u64,
}

#[cw_serde]
pub struct PendingUnbondingResponse {
    pub amount: Uint128,
    pub start_hint: Option<u64>,
}

#[cw_serde]
pub struct ActiveUnbondingsResponse {
    /// The active unbondings
    pub unbondings: Vec<UnbondingStatus>,
}

#[cw_serde]
pub struct StateResponse {
    /// Total amount of deposits in the vault
    pub total_deposits: Uint128,
    /// Total number of issued shares
    pub total_issued_shares: Uint128,
}

#[cw_serde]
pub struct ClaimableResponse {
    /// Amount of assets elligible for claiming
    pub amount: Uint128,
}

#[cw_serde]
pub struct UnderlyingAssetDecimalsResponse {
    pub decimals: Decimals,
}

#[cw_serde]
pub struct DepositAssetResponse {
    pub denom: String,
}

#[cw_serde]
pub struct SharesAssetResponse {
    pub denom: String,
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum QueryMsg {
    /// Returns the state of the vault, i.e. total shares issued & total deposit value
    #[returns(StateResponse)]
    State {},

    /// Returns the pending unbonding for the given address if present, otherwise the whole contract
    #[returns(PendingUnbondingResponse)]
    PendingUnbonding { address: Option<String> },

    /// Returns all the unbondings for the given address if present, otherwise the whole contract
    #[returns(ActiveUnbondingsResponse)]
    ActiveUnbondings { address: Option<String> },

    /// Returns the current claimable balance for the address
    #[returns(ClaimableResponse)]
    Claimable { address: String },

    /// Returns the underlying asset decimals that the vault deposit value is denominated with
    #[returns(UnderlyingAssetDecimalsResponse)]
    UnderlyingAssetDecimals {},

    /// Returns the asset that the vault accepts for deposit
    #[returns(DepositAssetResponse)]
    DepositAsset {},

    /// Returns the shares asset issued by the vault
    #[returns(SharesAssetResponse)]
    SharesAsset {},
}

fn handle_vault_deposit<Msg>(
    info: MessageInfo,
    vault: &dyn Vault,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let deposit_asset_coin = one_coin(&info)?;

    let CoreDepositResponse {
        cmds,
        deposit_value,
        issued_shares,
        total_shares_issued,
        total_deposits_value,
    } = vault.deposit(
        deposit_asset_coin.denom.into(),
        deposit_asset_coin.amount.u128(),
        info.sender.into_string().into(),
    )?;

    let data = to_json_binary(&DepositResponse {
        total_shares_issued: total_shares_issued.into(),
        total_deposits_value: total_deposits_value.into(),
        minted_shares: issued_shares.into(),
        deposit_value: deposit_value.into(),
    })?;

    Ok((cmds, Response::default().set_data(data)))
}

fn handle_vault_donation<Msg>(
    info: MessageInfo,
    vault: &dyn Vault,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let deposit_asset_coin = one_coin(&info)?;

    let cmd = vault.donate(
        deposit_asset_coin.denom.into(),
        deposit_asset_coin.amount.u128(),
    )?;

    Ok((vec![cmd.into()], Response::default()))
}

fn handle_vault_redemption<Msg>(
    info: MessageInfo,
    vault: &dyn Vault,
    recipient: String,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let redemption_asset_coin = one_coin(&info)?;

    let cmds = vault.redeem(
        redemption_asset_coin.denom.into(),
        redemption_asset_coin.amount.u128(),
        recipient.into(),
    )?;

    Ok((cmds, Response::default()))
}

fn handle_vault_start_unbond<Msg>(
    info: MessageInfo,
    vault: &dyn Vault,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    non_payable(&info)?;

    let cmds = vault.start_unbond()?;

    Ok((cmds, Response::default()))
}

fn handle_vault_claim<Msg>(
    info: MessageInfo,
    vault: &dyn Vault,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    non_payable(&info)?;

    let cmds = vault.claim(info.sender.into_string().into())?;

    Ok((cmds, Response::default()))
}

pub fn handle_execute_msg<Msg>(
    strategy: &dyn Strategy,
    unbonding_log: &dyn CoreUnbondingLog,
    mint: &dyn CoreSharesMint,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let vault = vault(strategy, unbonding_log, mint);

    match msg {
        ExecuteMsg::Deposit {} => handle_vault_deposit(info, &vault),
        ExecuteMsg::Donate {} => handle_vault_donation(info, &vault),
        ExecuteMsg::Redeem { recipient } => handle_vault_redemption(info, &vault, recipient),
        ExecuteMsg::StartUnbond {} => handle_vault_start_unbond(info, &vault),
        ExecuteMsg::Claim {} => handle_vault_claim(info, &vault),
    }
}

pub fn pending_unbonding(
    unbonding_log: &dyn CoreUnbondingLog,
    recipient: Option<String>,
) -> Result<PendingUnbondingResponse, StdError> {
    let pending_batch_id = pending_batch_id(unbonding_log);

    let amount = if let Some(recipient) = recipient {
        unbonding_log
            .unbonded_value_in_batch(&recipient, pending_batch_id)
            .unwrap_or_default()
            .into()
    } else {
        unbonding_log
            .batch_unbond_value(pending_batch_id)
            .unwrap_or_default()
            .into()
    };

    let start_hint = unbonding_log.pending_batch_hint(pending_batch_id);

    Ok(PendingUnbondingResponse { amount, start_hint })
}

pub fn account_active_unbondings(
    storage: &dyn Storage,
    unbonding_log: &dyn CoreUnbondingLog,
    env: &Env,
    account: &str,
) -> ActiveUnbondingsResponse {
    let mut unbondings = vec![];

    let Some(last_entered_batch) = unbonding_log.last_entered_batch(account) else {
        return ActiveUnbondingsResponse { unbondings };
    };

    let Some(last_committed_batch) = unbonding_log.last_committed_batch_id() else {
        return ActiveUnbondingsResponse { unbondings };
    };

    // determine the first batch to look at, i.e. latest committed batch entered by the account
    let mut current_batch = if last_entered_batch <= last_committed_batch {
        last_entered_batch
    } else {
        // last entered batch was the pending batch
        // so we're looking for the next latest batch
        let Some(previously_entered_batch) =
            storage.previously_entered_batch(account, last_entered_batch)
        else {
            // no previously entered batches, nothing left to do
            return ActiveUnbondingsResponse { unbondings };
        };

        previously_entered_batch
    };

    let now = env.block.time.seconds();

    loop {
        let UnbondEpoch { start, end } = unbonding_log
            .committed_batch_epoch(current_batch)
            .expect("always: epoch set for a committed batch");

        if end < now {
            break;
        }

        let amount = unbonding_log
            .unbonded_value_in_batch(account, current_batch)
            .expect("always: non-zero amount unbonded in committed batch")
            .into();

        unbondings.push(UnbondingStatus { amount, start, end });

        let Some(previously_entered_batch) =
            storage.previously_entered_batch(account, last_entered_batch)
        else {
            break;
        };

        current_batch = previously_entered_batch;
    }

    ActiveUnbondingsResponse { unbondings }
}

pub fn all_active_unbondings(
    unbonding_log: &dyn CoreUnbondingLog,
    env: &Env,
) -> ActiveUnbondingsResponse {
    let mut unbondings = vec![];

    let Some(last_committed_batch) = unbonding_log.last_committed_batch_id() else {
        return ActiveUnbondingsResponse { unbondings };
    };

    let now = env.block.time.seconds();

    for batch_id in (0..=last_committed_batch).rev() {
        let UnbondEpoch { start, end } = unbonding_log
            .committed_batch_epoch(batch_id)
            .expect("always: epoch set for a committed batch");

        if end < now {
            break;
        }

        let amount = unbonding_log
            .batch_unbond_value(batch_id)
            .expect("always: non-zero amount unbonded in committed batch")
            .into();

        unbondings.push(UnbondingStatus { amount, start, end })
    }

    ActiveUnbondingsResponse { unbondings }
}

pub fn handle_query_msg(
    storage: &dyn Storage,
    strategy: &dyn Strategy,
    unbonding_log: &dyn CoreUnbondingLog,
    mint: &dyn CoreSharesMint,
    env: &Env,
    msg: QueryMsg,
) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::State {} => {
            let total_shares_issue = mint.total_shares_issued();

            let total_deposits = offset_total_deposits_value(strategy, unbonding_log);

            to_json_binary(&StateResponse {
                total_deposits: total_deposits.into(),
                total_issued_shares: total_shares_issue.into(),
            })
        }

        QueryMsg::PendingUnbonding { address } => {
            let pending_unbonding = pending_unbonding(unbonding_log, address)?;

            to_json_binary(&pending_unbonding)
        }

        QueryMsg::ActiveUnbondings { address } => {
            let active_unbondings = if let Some(address) = address {
                account_active_unbondings(storage, unbonding_log, env, &address)
            } else {
                all_active_unbondings(unbonding_log, env)
            };

            to_json_binary(&active_unbondings)
        }

        QueryMsg::Claimable { address } => {
            let amount = ClaimableBatchIter::new(&address, unbonding_log, strategy)
                .map(|(amount, _)| amount)
                .sum::<u128>()
                .into();

            to_json_binary(&ClaimableResponse { amount })
        }

        QueryMsg::UnderlyingAssetDecimals {} => to_json_binary(&UnderlyingAssetDecimalsResponse {
            decimals: strategy.underlying_asset_decimals(),
        }),

        QueryMsg::DepositAsset {} => to_json_binary(&DepositAssetResponse {
            denom: strategy.deposit_asset().into_string(),
        }),

        QueryMsg::SharesAsset {} => to_json_binary(&SharesAssetResponse {
            denom: mint.shares_asset().into_string(),
        }),
    }
}
