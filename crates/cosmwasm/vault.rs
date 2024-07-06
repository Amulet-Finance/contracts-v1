pub mod mint;
pub mod unbonding_log;

use amulet_core::{
    vault::{
        offset_total_deposits_value, pending_batch_id, vault, BatchId, ClaimAmount,
        ClaimableBatchIter, Cmd, DepositAmount, DepositResponse as CoreDepositResponse,
        DepositValue, Error as CoreVaultError, MintCmd, SharesAmount, SharesMint as CoreSharesMint,
        Strategy, StrategyCmd, UnbondEpoch, UnbondingLog as CoreUnbondingLog, UnbondingLogSet,
        Vault,
    },
    Decimals,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, Binary, Env, MessageInfo, Response, StdError, Storage, Uint128,
};
use cw_utils::{nonpayable, one_coin, PaymentError};
use strum::IntoStaticStr;

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
#[derive(IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
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

impl ExecuteMsg {
    /// A string representing the message 'kind'
    pub fn kind(&self) -> &'static str {
        // relies on deriving strum::IntoStaticStr
        self.into()
    }
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
#[derive(Default)]
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
pub struct UnbondingLogMetadata {
    pub last_committed_batch_id: Option<BatchId>,
    pub first_entered_batch: Option<BatchId>,
    pub last_entered_batch: Option<BatchId>,
    pub last_claimed_batch: Option<BatchId>,
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

    /// Returns all the unbondings for the given address if present, otherwise the whole contract.
    /// The unbondings are in descending order according to the epoch start and will only contain up to `limit` entries, if provided
    #[returns(ActiveUnbondingsResponse)]
    ActiveUnbondings {
        address: Option<String>,
        limit: Option<u32>,
    },

    /// Returns all the unbonding log metadata for the given address
    #[returns(UnbondingLogMetadata)]
    UnbondingLogMetadata { address: String },

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
    response: &mut Response<Msg>,
) -> Result<Vec<Cmd>, Error> {
    let deposit_asset_coin = one_coin(&info)?;

    let CoreDepositResponse {
        cmds,
        deposit_value,
        issued_shares,
        total_shares_issued,
        total_deposits_value,
    } = vault.deposit(
        deposit_asset_coin.denom.into(),
        DepositAmount(deposit_asset_coin.amount.u128()),
        info.sender.into_string().into(),
    )?;

    let data = to_json_binary(&DepositResponse {
        total_shares_issued: total_shares_issued.0.into(),
        total_deposits_value: total_deposits_value.0.into(),
        minted_shares: issued_shares.0.into(),
        deposit_value: deposit_value.0.into(),
    })?;

    response.data = Some(data);

    Ok(cmds)
}

fn handle_vault_donation(info: MessageInfo, vault: &dyn Vault) -> Result<Vec<Cmd>, Error> {
    let deposit_asset_coin = one_coin(&info)?;

    let cmd = vault.donate(
        deposit_asset_coin.denom.into(),
        DepositAmount(deposit_asset_coin.amount.u128()),
    )?;

    Ok(vec![cmd.into()])
}

fn handle_vault_redemption(
    info: MessageInfo,
    vault: &dyn Vault,
    recipient: String,
) -> Result<Vec<Cmd>, Error> {
    let redemption_asset_coin = one_coin(&info)?;

    let cmds = vault.redeem(
        redemption_asset_coin.denom.into(),
        SharesAmount(redemption_asset_coin.amount.u128()),
        recipient.into(),
    )?;

    Ok(cmds)
}

fn handle_vault_start_unbond(info: MessageInfo, vault: &dyn Vault) -> Result<Vec<Cmd>, Error> {
    nonpayable(&info)?;

    let cmds = vault.start_unbond()?;

    Ok(cmds)
}

fn handle_vault_claim(info: MessageInfo, vault: &dyn Vault) -> Result<Vec<Cmd>, Error> {
    nonpayable(&info)?;

    let cmds = vault.claim(info.sender.into_string().into())?;

    Ok(cmds)
}

struct AttrsBuilder<'a, Msg>(&'a mut Response<Msg>);

impl<'a, Msg> AttrsBuilder<'a, Msg> {
    fn add_attr(&mut self, k: &str, v: impl ToString) -> &mut Self {
        self.0.attributes.push((k, v.to_string()).into());
        self
    }

    fn add_kind(&mut self, kind: &str) -> &mut Self {
        self.add_attr("kind", kind)
    }

    fn add_recipient(&mut self, recipient: &str) -> &mut Self {
        self.add_attr("recipient", recipient)
    }

    fn add_amount(&mut self, amount: impl ToString) -> &mut Self {
        self.add_attr("amount", amount)
    }
}

fn add_msg_attrs<Msg>(msg: &ExecuteMsg, info: &MessageInfo, response: &mut Response<Msg>) {
    let mut attrs = AttrsBuilder(response);

    attrs.add_kind(msg.kind());

    match msg {
        ExecuteMsg::Deposit {} | ExecuteMsg::Claim {} => attrs.add_recipient(info.sender.as_str()),
        ExecuteMsg::Donate {} => attrs.add_attr("donor", &info.sender),
        ExecuteMsg::Redeem { recipient } => attrs.add_recipient(recipient),
        _ => &mut attrs,
    };
}

fn add_cmd_attrs<Msg>(cmds: &[Cmd], response: &mut Response<Msg>) {
    let mut attrs = AttrsBuilder(response);

    for cmd in cmds {
        match cmd {
            Cmd::Mint(cmd) => match cmd {
                MintCmd::Mint {
                    amount: SharesAmount(amount),
                    ..
                } => attrs.add_attr("mint_shares", amount),
                MintCmd::Burn {
                    amount: SharesAmount(amount),
                } => attrs.add_attr("burn_shares", amount),
            },
            Cmd::Strategy(cmd) => match cmd {
                StrategyCmd::Deposit {
                    amount: DepositAmount(amount),
                }
                | StrategyCmd::SendClaimed {
                    amount: ClaimAmount(amount),
                    ..
                } => attrs.add_amount(amount),
                StrategyCmd::Unbond {
                    value: DepositValue(value),
                } => attrs.add_attr("unbond_value", value),
            },
            Cmd::UnbondingLog(cmd) => match cmd {
                UnbondingLogSet::LastCommittedBatchId(batch) => {
                    attrs.add_attr("batch_committed", batch)
                }
                UnbondingLogSet::BatchTotalUnbondValue {
                    value: DepositValue(value),
                    ..
                } => attrs.add_attr("batch_total_value", value),
                UnbondingLogSet::BatchClaimableAmount {
                    amount: ClaimAmount(amount),
                    ..
                } => attrs.add_attr("batch_total_claim", amount),
                UnbondingLogSet::BatchHint { hint, .. } => attrs.add_attr("batch_start_hint", hint),
                UnbondingLogSet::BatchEpoch { epoch, .. } => attrs
                    .add_attr("batch_start", epoch.start)
                    .add_attr("batch_end", epoch.end),
                UnbondingLogSet::UnbondedValueInBatch {
                    value: DepositValue(value),
                    ..
                } => attrs.add_attr("batch_recipient_value", value),
                _ => &mut attrs,
            },
        };
    }

    // add batch attribute only once
    if let Some(batch) = cmds.iter().find_map(|cmd| match cmd {
        Cmd::UnbondingLog(
            UnbondingLogSet::BatchTotalUnbondValue { batch, .. }
            | UnbondingLogSet::BatchClaimableAmount { batch, .. }
            | UnbondingLogSet::UnbondedValueInBatch { batch, .. }
            | UnbondingLogSet::BatchHint { batch, .. }
            | UnbondingLogSet::BatchEpoch { batch, .. },
        ) => Some(batch),
        _ => None,
    }) {
        attrs.add_attr("batch", batch);
    }
}

pub fn handle_execute_msg<Msg>(
    strategy: &dyn Strategy,
    unbonding_log: &dyn CoreUnbondingLog,
    mint: &dyn CoreSharesMint,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let vault = vault(strategy, unbonding_log, mint);

    let mut response = Response::default();

    add_msg_attrs(&msg, &info, &mut response);

    let cmds = match msg {
        ExecuteMsg::Deposit {} => handle_vault_deposit(info, &vault, &mut response)?,
        ExecuteMsg::Donate {} => handle_vault_donation(info, &vault)?,
        ExecuteMsg::Redeem { recipient } => handle_vault_redemption(info, &vault, recipient)?,
        ExecuteMsg::StartUnbond {} => handle_vault_start_unbond(info, &vault)?,
        ExecuteMsg::Claim {} => handle_vault_claim(info, &vault)?,
    };

    add_cmd_attrs(&cmds, &mut response);

    Ok((cmds, response))
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
            .0
            .into()
    } else {
        unbonding_log
            .batch_unbond_value(pending_batch_id)
            .unwrap_or_default()
            .0
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
    limit: Option<u32>,
) -> ActiveUnbondingsResponse {
    let Some(last_entered_batch) = unbonding_log.last_entered_batch(account) else {
        return ActiveUnbondingsResponse::default();
    };

    let Some(last_committed_batch) = unbonding_log.last_committed_batch_id() else {
        return ActiveUnbondingsResponse::default();
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
            return ActiveUnbondingsResponse::default();
        };

        previously_entered_batch
    };

    let now = env.block.time.seconds();

    let mut unbondings = vec![];

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
            .0
            .into();

        unbondings.push(UnbondingStatus { amount, start, end });

        if let Some(limit) = limit {
            if unbondings.len() == limit as usize {
                break;
            }
        }

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
    limit: Option<u32>,
) -> ActiveUnbondingsResponse {
    let Some(last_committed_batch) = unbonding_log.last_committed_batch_id() else {
        return ActiveUnbondingsResponse::default();
    };

    let now = env.block.time.seconds();

    let mut unbondings = vec![];

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
            .0
            .into();

        unbondings.push(UnbondingStatus { amount, start, end });

        if let Some(limit) = limit {
            if unbondings.len() == limit as usize {
                break;
            }
        }
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
                total_deposits: total_deposits.0.into(),
                total_issued_shares: total_shares_issue.0.into(),
            })
        }

        QueryMsg::PendingUnbonding { address } => {
            let pending_unbonding = pending_unbonding(unbonding_log, address)?;

            to_json_binary(&pending_unbonding)
        }

        QueryMsg::ActiveUnbondings { address, limit } => {
            let active_unbondings = if let Some(address) = address {
                account_active_unbondings(storage, unbonding_log, env, &address, limit)
            } else {
                all_active_unbondings(unbonding_log, env, limit)
            };

            to_json_binary(&active_unbondings)
        }

        QueryMsg::UnbondingLogMetadata { address } => to_json_binary(&UnbondingLogMetadata {
            last_committed_batch_id: unbonding_log.last_committed_batch_id(),
            first_entered_batch: unbonding_log.first_entered_batch(&address),
            last_entered_batch: unbonding_log.last_entered_batch(&address),
            last_claimed_batch: unbonding_log.last_claimed_batch(&address),
        }),

        QueryMsg::Claimable { address } => {
            let amount = ClaimableBatchIter::new(&address, unbonding_log, strategy)
                .map(|(ClaimAmount(amount), _)| amount)
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
