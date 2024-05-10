use crate::{cmds, Asset, Decimals, Rate, Recipient};

// TODO: Newtype candidates
/// An amount of deposit assets
pub type DepositAmount = u128;
/// The value of a deposit in terms of the *underlying* asset
pub type DepositValue = u128;
/// An amount of shares assets
pub type SharesAmount = u128;
/// An amount of claimable assets
pub type ClaimAmount = u128;
/// The total number of shares issued
pub type TotalSharesIssued = u128;
/// The total value of deposits in terms of the *underlying* asset
pub type TotalDepositsValue = u128;
pub type Instant = u64;
pub type Now = u64;
pub type Hint = u64;
pub type BatchId = u64;

pub const SHARES_DECIMAL_PLACES: Decimals = 18;

#[derive(Debug, Clone, Copy)]
pub enum UnbondReadyStatus {
    /// Unbonding is ready to be started
    Ready {
        /// The amount claimable in terms of the unbond-able asset (this could be different to the unbond amount requested, e.g. decimals)
        amount: ClaimAmount,
        /// The expected unbonding epoch
        epoch: UnbondEpoch,
    },

    /// Unbonding can be only be started later (there is a hint if possible)
    Later(Option<Hint>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnbondEpoch {
    pub start: Instant,
    pub end: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MintCmd {
    /// Instructs the mint to create an `amount` of shares to be received by the `recipient`, increasing the number of total issued shares
    Mint {
        amount: SharesAmount,
        recipient: Recipient,
    },

    /// Instructs the mint to destroy an `amount` of shares, decreasing the number of total issued shares.
    /// NOTE: It is assumed that the downstream user of the library has 'taken custody' of the shares to burn.
    Burn { amount: SharesAmount },
}

pub trait SharesMint {
    /// Returns the total issued shares
    fn total_shares_issued(&self) -> TotalSharesIssued;

    /// Returns asset that the strategy issues as shares
    fn shares_asset(&self) -> Asset;
}

#[derive(Debug, PartialEq, Eq)]
pub enum StrategyCmd {
    /// Deposit an `amount` of deposit assets into the strategy
    Deposit { amount: DepositAmount },

    /// Unbond an amount of deposited assets in equal `value` to the underlying asset
    /// NOTE: It is assumed that when an unbond is started, that the reported `total_deposits_value` by the strategy does
    /// *NOT* include the unbonded amount of deposited assets.
    Unbond { value: DepositValue },

    /// Send an `amount` of assets that should be undbonded from the strategy to the `recipient`
    SendClaimed {
        amount: ClaimAmount,
        recipient: Recipient,
    },
}

pub trait Strategy {
    /// Returns current instant in terms of unbonding time
    fn now(&self) -> Now;

    /// Returns asset that the strategy can accept for deposits
    fn deposit_asset(&self) -> Asset;

    /// Returns the decimals used by the underlying asset
    fn underlying_asset_decimals(&self) -> Decimals;

    /// Returns the total strategy deposits valued in terms of the underlying asset.
    fn total_deposits_value(&self) -> TotalDepositsValue;

    /// Returns the deposit amount valued in terms of the underlying asset, which could be different to the *deposit asset* amount.
    fn deposit_value(&self, amount: DepositAmount) -> DepositValue;

    /// Returns the `UnbondReadyStatus::Later(_)` with an optional start hint if unbonding is not yet possible,
    /// otherwise `UnbondReadyStatus::Ready { amount, epoch }`.
    fn unbond(&self, value: DepositValue) -> UnbondReadyStatus;
}

#[derive(Debug, PartialEq, Eq)]
pub enum UnbondingLogSet {
    /// Set the last committed batch ID
    LastCommittedBatchId(BatchId),

    /// Set the total amount being unbonded in a batch
    BatchTotalUnbondValue { batch: BatchId, value: DepositValue },

    /// Set the claimable amount in a committed batch
    BatchClaimableAmount { batch: BatchId, amount: ClaimAmount },

    /// Set the hint for the pending batch
    BatchHint { batch: BatchId, hint: Hint },

    /// Set the epoch for a committed batch
    BatchEpoch { batch: BatchId, epoch: UnbondEpoch },

    /// Set the recipient's first entered batch ID
    FirstEnteredBatch {
        recipient: Recipient,
        batch: BatchId,
    },

    /// Set the recipient's last entered batch ID
    LastEnteredBatch {
        recipient: Recipient,
        batch: BatchId,
    },

    /// Set the recipient's next batch ID for a batch
    NextEnteredBatch {
        recipient: Recipient,
        previous: BatchId,
        next: BatchId,
    },

    /// Set the recipient's last claimed batch ID
    LastClaimedBatch {
        recipient: Recipient,
        batch: BatchId,
    },

    /// Set the amount a recipient is unbonding in a batch
    UnbondedValueInBatch {
        recipient: Recipient,
        batch: BatchId,
        value: DepositValue,
    },
}

pub trait UnbondingLog {
    /// Returns the last committed batch id, if one has been set.
    fn last_committed_batch_id(&self) -> Option<BatchId>;

    /// Returns the total value of assets unbonded in a batch, if one has been set.
    fn batch_unbond_value(&self, batch: BatchId) -> Option<DepositValue>;

    /// Returns the claimable amount assigned to a batch id, if one has been set.
    fn batch_claimable_amount(&self, batch: BatchId) -> Option<ClaimAmount>;

    /// Returns the start hint assigned to a pending batch id, if one has been set.
    fn pending_batch_hint(&self, batch: BatchId) -> Option<Hint>;

    /// Returns the unbond epoch assigned to a batch id, if one has been set.
    fn committed_batch_epoch(&self, batch: BatchId) -> Option<UnbondEpoch>;

    /// Returns batch id of the first batch the recipient entered, if one has been set.
    fn first_entered_batch(&self, recipient: &str) -> Option<BatchId>;

    /// Returns batch id of the last batch the recipient entered, if one has been set.
    fn last_entered_batch(&self, recipient: &str) -> Option<BatchId>;

    /// Returns batch id of the batch the recipient entered *after* the given `batch`, if one has been set.
    fn next_entered_batch(&self, recipient: &str, batch: BatchId) -> Option<BatchId>;

    /// Returns batch id of the last batch successfully claimed by the recipient, if one has been set.
    fn last_claimed_batch(&self, recipient: &str) -> Option<BatchId>;

    /// Returns unbonded value belonging to a recipient in a batch, if any has been set.
    fn unbonded_value_in_batch(&self, recipient: &str, batch: BatchId) -> Option<DepositValue>;
}

#[derive(Debug, Clone, Copy)]
pub struct RedemptionRate {
    total_shares_issued: TotalSharesIssued,
    total_deposits_value: TotalDepositsValue,
}

impl RedemptionRate {
    pub fn new(
        total_shares_issued: TotalSharesIssued,
        total_deposits_value: TotalDepositsValue,
    ) -> Option<Self> {
        if total_shares_issued == 0 || total_deposits_value == 0 {
            return None;
        }

        Some(Self {
            total_shares_issued,
            total_deposits_value,
        })
    }

    pub fn checked_shares_to_deposits(&self, shares_amount: SharesAmount) -> Option<DepositAmount> {
        Rate::from_ratio(shares_amount, self.total_shares_issued)
            .and_then(|rate| rate.apply_u128(self.total_deposits_value))
    }

    pub fn checked_deposits_to_shares(&self, deposit_value: DepositValue) -> Option<SharesAmount> {
        Rate::from_ratio(deposit_value, self.total_deposits_value)
            .and_then(|rate| rate.apply_u128(self.total_shares_issued))
    }

    fn overflow_panic(self, shares_or_deposits: &str, amount: u128) -> ! {
        panic!(
            "overflow converting {amount} to {shares_or_deposits}. total_shares_issued = {}, total_deposit_value = {}", 
            self.total_shares_issued,
            self.total_deposits_value
        );
    }

    pub fn shares_to_deposits(&self, shares_amount: SharesAmount) -> DepositAmount {
        self.checked_shares_to_deposits(shares_amount)
            .unwrap_or_else(|| self.overflow_panic("deposits", shares_amount))
    }

    pub fn deposits_to_shares(&self, deposit_amount: DepositAmount) -> SharesAmount {
        self.checked_deposits_to_shares(deposit_amount)
            .unwrap_or_else(|| self.overflow_panic("shares", deposit_amount))
    }
}

pub fn pending_batch_id(unbonding_log: &dyn UnbondingLog) -> BatchId {
    unbonding_log
        .last_committed_batch_id()
        // overflows in 584 billion years if 1 unbond per second
        .map_or(0, |batch_id| batch_id + 1)
}

/// Offsets the total deposit value reported by the strategy by the value of any deposits pending unbonding
pub fn offset_total_deposits_value(
    strategy: &dyn Strategy,
    unbonding_log: &dyn UnbondingLog,
) -> TotalDepositsValue {
    let pending_batch_id = pending_batch_id(unbonding_log);

    let offset = unbonding_log
        .batch_unbond_value(pending_batch_id)
        .unwrap_or_default();

    strategy
        .total_deposits_value()
        .checked_sub(offset)
        .expect("pending unbond <= total deposits")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Cmd {
    Mint(MintCmd),
    Strategy(StrategyCmd),
    UnbondingLog(UnbondingLogSet),
}

// extend a Vec<Cmd> type to add a builder method to chain adding different commands
trait CmdVecExt {
    fn add_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self;
}

impl CmdVecExt for Vec<Cmd> {
    fn add_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self {
        self.push(cmd.into());
        self
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid deposit asset")]
    InvalidDepositAsset,

    #[error("cannot deposit zero")]
    CannotDepositZero,

    /// Due to the ever increasing redemption rate: (total_deposits / total_shares_issued)
    /// It is logically possible that a single share unit is valued higher than a single deposit asset unit.
    /// In practice this is highly unlikely due to shares using 18 decimal places.
    #[error("deposit too small")]
    DepositTooSmall,

    /// It is logically possible that a deposit is so large that the resulting amount of total or minted shares is
    /// larger than the max representable value.
    /// In practice this is highly unlikely due to the use of unsigned 128-bit integers for balances.
    #[error("deposit too large")]
    DepositTooLarge,

    #[error("cannot deposit in total loss state")]
    CannotDepositInTotalLossState,

    #[error("invalid donation asset")]
    InvalidDonationAsset,

    #[error("cannot donate zero")]
    CannotDonateZero,

    #[error("cannot redeem zero")]
    CannotRedeemZero,

    #[error("invalid redemption asset")]
    InvalidRedemptionAsset,

    /// It is logically possible that there is a total loss of deposits from the vault in which case the
    /// value of issued shares is zero.
    #[error("no deposits to redeem")]
    NoDepositsToRedeem,

    /// It is logically possible that a single share unit is valued less than a single deposit asset unit.
    /// In practice this is highly likely due to shares using 18 decimal places.
    #[error("redemption too small")]
    RedemptionTooSmall,

    /// It is logically possible that a the total amount unbonded in a batch is larger than the max representable value.
    /// In practice this is highly unlikely due to the use of unsigned 128-bit integers for balances.
    #[error("redemption too large")]
    RedemptionTooLarge,

    #[error("nothing to claim")]
    NothingToClaim,

    #[error("nothing to unbond")]
    NothingToUnbond,

    #[error("unbond not ready")]
    UnbondNotReady,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DepositResponse {
    pub cmds: Vec<Cmd>,
    pub deposit_value: DepositValue,
    pub issued_shares: SharesAmount,
    pub total_shares_issued: TotalSharesIssued,
    pub total_deposits_value: TotalDepositsValue,
}

pub trait Vault {
    fn deposit(
        &self,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        mint_recipient: Recipient,
    ) -> Result<DepositResponse, Error>;

    fn donate(
        &self,
        donate_asset: Asset,
        donate_amount: DepositAmount,
    ) -> Result<StrategyCmd, Error>;

    fn redeem(
        &self,
        shares_asset: Asset,
        shares_amount: SharesAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn claim(&self, recipient: Recipient) -> Result<Vec<Cmd>, Error>;

    fn start_unbond(&self) -> Result<Vec<Cmd>, Error>;
}

pub struct VaultImpl<'a> {
    strategy: &'a dyn Strategy,
    unbonding_log: &'a dyn UnbondingLog,
    mint: &'a dyn SharesMint,
}

pub fn vault<'a>(
    strategy: &'a dyn Strategy,
    unbonding_log: &'a dyn UnbondingLog,
    mint: &'a dyn SharesMint,
) -> VaultImpl<'a> {
    VaultImpl {
        strategy,
        unbonding_log,
        mint,
    }
}

impl<'a> VaultImpl<'a> {
    fn pending_batch_id(&self) -> BatchId {
        pending_batch_id(self.unbonding_log)
    }

    fn offset_total_deposits_value(&self) -> TotalDepositsValue {
        offset_total_deposits_value(self.strategy, self.unbonding_log)
    }
}

pub struct ClaimableBatchIter<'a> {
    recipient: &'a str,
    unbonding_log: &'a dyn UnbondingLog,
    now: Now,
    highest_id: Option<BatchId>,
    next_id: Option<BatchId>,
}

impl<'a> ClaimableBatchIter<'a> {
    pub fn new(
        recipient: &'a str,
        unbonding_log: &'a dyn UnbondingLog,
        strategy: &dyn Strategy,
    ) -> Self {
        Self {
            recipient,
            unbonding_log,
            now: strategy.now(),
            highest_id: None,
            next_id: None,
        }
    }

    fn try_start(&mut self) -> Option<(ClaimAmount, BatchId)> {
        let last_entered = self.unbonding_log.last_entered_batch(self.recipient)?;

        let last_committed_batch = self.unbonding_log.last_committed_batch_id()?;

        let highest_id = if last_entered > last_committed_batch {
            last_committed_batch
        } else {
            last_entered
        };

        self.highest_id = Some(highest_id);

        let first_id = match self.unbonding_log.last_claimed_batch(self.recipient) {
            Some(last_claimed_batch) => {
                let next_id = self
                    .unbonding_log
                    .next_entered_batch(self.recipient, last_claimed_batch)?;

                if next_id > highest_id {
                    return None;
                }

                next_id
            }

            None => self
                .unbonding_log
                .first_entered_batch(self.recipient)
                .expect("first entered batch id present if last entered batch id present"),
        };

        // first id could be the currently pending batch
        if first_id > highest_id {
            return None;
        }

        self.try_batch(highest_id, first_id)
    }

    fn try_batch(
        &mut self,
        highest_id: BatchId,
        batch_id: BatchId,
    ) -> Option<(ClaimAmount, BatchId)> {
        let epoch = self
            .unbonding_log
            .committed_batch_epoch(batch_id)
            .expect("batch id <= highest batch id < pending batch id");

        if epoch.end > self.now {
            return None;
        }

        self.next_id = self
            .unbonding_log
            .next_entered_batch(self.recipient, batch_id)
            .filter(|next_batch_id| *next_batch_id <= highest_id);

        let ((recipient_unbonded, total_unbonded), total_claimable) = self
            .unbonding_log
            .unbonded_value_in_batch(self.recipient, batch_id)
            .zip(self.unbonding_log.batch_unbond_value(batch_id))
            .zip(self.unbonding_log.batch_claimable_amount(batch_id))
            .expect("batch has been entered by recipient and committed");

        let claim_amount = Rate::from_ratio(recipient_unbonded, total_unbonded)
            .expect("unbonded non-zero amount")
            .apply_u128(total_claimable)
            .expect("recipient unbonded <= total unbonded");

        Some((claim_amount, batch_id))
    }
}

impl<'a> Iterator for ClaimableBatchIter<'a> {
    type Item = (ClaimAmount, BatchId);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(highest_id) = self.highest_id else {
            return self.try_start();
        };

        let Some(next_batch) = self.next_id else {
            return None;
        };

        self.try_batch(highest_id, next_batch)
    }
}

impl<'a> Vault for VaultImpl<'a> {
    fn deposit(
        &self,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        mint_recipient: Recipient,
    ) -> Result<DepositResponse, Error> {
        if deposit_amount == 0 {
            return Err(Error::CannotDepositZero);
        }

        if deposit_asset != self.strategy.deposit_asset() {
            return Err(Error::InvalidDepositAsset);
        }

        let previous_total_deposits_value = self.offset_total_deposits_value();

        // Value the deposit in terms of the underlying strategy token
        let deposit_value = self.strategy.deposit_value(deposit_amount);

        let total_shares_issued = self.mint.total_shares_issued();

        let total_deposits_value = previous_total_deposits_value
            .checked_add(deposit_value)
            .ok_or(Error::DepositTooLarge)?;

        let deposit_cmd = StrategyCmd::Deposit {
            amount: deposit_amount,
        };

        let Some(redemption_rate) =
            RedemptionRate::new(total_shares_issued, previous_total_deposits_value)
        else {
            // It is logically possible that a total loss occurs in the strategy and there are >0 issued shares but 0 deposits
            // In this case, new deposits should not be allowed as it would overwrite the issued shares which could be made
            // whole via a donation.
            if total_shares_issued != 0 {
                return Err(Error::CannotDepositInTotalLossState);
            }

            let underlying_asset_decimals = self.strategy.underlying_asset_decimals();

            if underlying_asset_decimals == SHARES_DECIMAL_PLACES {
                return Ok(DepositResponse {
                    cmds: cmds![
                        deposit_cmd,
                        MintCmd::Mint {
                            amount: total_deposits_value,
                            recipient: mint_recipient,
                        }
                    ],
                    deposit_value,
                    issued_shares: total_deposits_value,
                    total_shares_issued: total_deposits_value,
                    total_deposits_value,
                });
            }

            assert!(
                underlying_asset_decimals <= SHARES_DECIMAL_PLACES,
                "underlying asset decimals cannot be greater than shares decimals"
            );

            // initial share issuance == inital deposit amount normalized to Self::SHARE_DECIMALS
            let scaler = 10u128.pow(SHARES_DECIMAL_PLACES - underlying_asset_decimals);

            let mint_shares = total_deposits_value
                .checked_mul(scaler)
                .ok_or(Error::DepositTooLarge)?;

            return Ok(DepositResponse {
                cmds: cmds![
                    deposit_cmd,
                    MintCmd::Mint {
                        amount: mint_shares,
                        recipient: mint_recipient,
                    }
                ],
                deposit_value,
                issued_shares: mint_shares,
                total_shares_issued: mint_shares,
                total_deposits_value,
            });
        };

        let mint_shares = redemption_rate
            .checked_deposits_to_shares(deposit_value)
            .ok_or(Error::DepositTooLarge)?;

        if mint_shares == 0 {
            return Err(Error::DepositTooSmall);
        }

        let mint_shares_value = redemption_rate.shares_to_deposits(mint_shares);

        let total_shares_issued = total_shares_issued
            .checked_add(mint_shares)
            .ok_or(Error::DepositTooLarge)?;

        Ok(DepositResponse {
            cmds: cmds![
                deposit_cmd,
                MintCmd::Mint {
                    amount: mint_shares,
                    recipient: mint_recipient,
                }
            ],
            deposit_value: mint_shares_value,
            issued_shares: mint_shares,
            total_shares_issued,
            total_deposits_value,
        })
    }

    fn donate(
        &self,
        donate_asset: Asset,
        donate_amount: DepositAmount,
    ) -> Result<StrategyCmd, Error> {
        if donate_amount == 0 {
            return Err(Error::CannotDonateZero);
        }

        if donate_asset != self.strategy.deposit_asset() {
            return Err(Error::InvalidDonationAsset);
        }

        Ok(StrategyCmd::Deposit {
            amount: donate_amount,
        })
    }

    fn redeem(
        &self,
        shares_asset: Asset,
        shares_amount: SharesAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if shares_asset != self.mint.shares_asset() {
            return Err(Error::InvalidRedemptionAsset);
        }

        let total_shares_issued = self.mint.total_shares_issued();

        // NOTE: The following assertions are present as it is the downstream user of the library's responsibility
        // to be 'in custody' of the shares that are to be redeemed before making this request.
        assert!(
            shares_amount <= total_shares_issued,
            "cannot redeem more shares than have been issued"
        );
        assert!(
            total_shares_issued > 0,
            "shares must have been issued in order for shares to be redeemed"
        );

        if shares_amount == 0 {
            return Err(Error::CannotRedeemZero);
        }

        // total deposits value less the value of pending unbondings
        let offset_total_deposits_value = self.offset_total_deposits_value();

        let redemption_rate = RedemptionRate::new(total_shares_issued, offset_total_deposits_value)
            .ok_or(Error::NoDepositsToRedeem)?;

        let unbond_value = redemption_rate
            .checked_shares_to_deposits(shares_amount)
            .ok_or(Error::RedemptionTooSmall)?;

        let pending_batch_id = self.pending_batch_id();

        let total_unbond_value = self
            .unbonding_log
            .batch_unbond_value(pending_batch_id)
            .map_or(Some(unbond_value), |existing_value| {
                existing_value.checked_add(unbond_value)
            })
            .ok_or(Error::RedemptionTooLarge)?;

        let recipient_unbond_amount = self
            .unbonding_log
            .unbonded_value_in_batch(&recipient, pending_batch_id)
            .map_or(Some(unbond_value), |existing_value| {
                existing_value.checked_add(unbond_value)
            })
            .ok_or(Error::RedemptionTooLarge)?;

        let mut cmds: Vec<Cmd> = cmds![
            UnbondingLogSet::BatchTotalUnbondValue {
                batch: pending_batch_id,
                value: total_unbond_value,
            },
            UnbondingLogSet::UnbondedValueInBatch {
                recipient: recipient.clone(),
                batch: pending_batch_id,
                value: recipient_unbond_amount
            },
            MintCmd::Burn {
                amount: shares_amount
            }
        ];

        match self.unbonding_log.last_entered_batch(&recipient) {
            Some(batch_id) if batch_id != pending_batch_id => {
                cmds.add_cmd(UnbondingLogSet::LastEnteredBatch {
                    recipient: recipient.clone(),
                    batch: pending_batch_id,
                })
                .add_cmd(UnbondingLogSet::NextEnteredBatch {
                    recipient: recipient.clone(),
                    previous: batch_id,
                    next: pending_batch_id,
                });
            }

            None => {
                cmds.add_cmd(UnbondingLogSet::LastEnteredBatch {
                    recipient: recipient.clone(),
                    batch: pending_batch_id,
                })
                .add_cmd(UnbondingLogSet::FirstEnteredBatch {
                    recipient: recipient.clone(),
                    batch: pending_batch_id,
                });
            }

            _ => {}
        }

        match self.strategy.unbond(total_unbond_value) {
            UnbondReadyStatus::Ready { amount, epoch } => {
                cmds.add_cmd(UnbondingLogSet::LastCommittedBatchId(pending_batch_id))
                    .add_cmd(UnbondingLogSet::BatchClaimableAmount {
                        batch: pending_batch_id,
                        amount,
                    })
                    .add_cmd(UnbondingLogSet::BatchEpoch {
                        batch: pending_batch_id,
                        epoch,
                    })
                    .add_cmd(StrategyCmd::Unbond {
                        value: unbond_value,
                    });
            }

            UnbondReadyStatus::Later(Some(hint)) => {
                cmds.add_cmd(UnbondingLogSet::BatchHint {
                    batch: pending_batch_id,
                    hint,
                });
            }
            _ => {}
        }

        Ok(cmds)
    }

    fn claim(&self, recipient: Recipient) -> Result<Vec<Cmd>, Error> {
        let mut total_claimable_amount = 0u128;
        let mut last_claimed_id: Option<BatchId> = None;

        let iter = ClaimableBatchIter::new(&recipient, self.unbonding_log, self.strategy);

        for (amount, id) in iter {
            // It it logically possible that a recipient's total claimable balance exceeds the max representable value.
            // In this case the claim is split by stopping the accumulation at the previous iteration.
            let Some(total) = total_claimable_amount.checked_add(amount) else {
                break;
            };

            total_claimable_amount = total;
            last_claimed_id = Some(id);
        }

        let last_claimed_id = last_claimed_id.ok_or(Error::NothingToClaim)?;

        Ok(cmds![
            UnbondingLogSet::LastClaimedBatch {
                recipient: recipient.clone(),
                batch: last_claimed_id
            },
            StrategyCmd::SendClaimed {
                amount: total_claimable_amount,
                recipient
            }
        ])
    }

    fn start_unbond(&self) -> Result<Vec<Cmd>, Error> {
        let pending_batch_id = self.pending_batch_id();

        let pending_unbond_value = self
            .unbonding_log
            .batch_unbond_value(pending_batch_id)
            .ok_or(Error::NothingToUnbond)?;

        let UnbondReadyStatus::Ready { amount, epoch } = self.strategy.unbond(pending_unbond_value)
        else {
            return Err(Error::UnbondNotReady);
        };

        Ok(cmds![
            UnbondingLogSet::LastCommittedBatchId(pending_batch_id),
            UnbondingLogSet::BatchClaimableAmount {
                batch: pending_batch_id,
                amount,
            },
            UnbondingLogSet::BatchEpoch {
                batch: pending_batch_id,
                epoch,
            },
            StrategyCmd::Unbond {
                value: pending_unbond_value,
            }
        ])
    }
}

impl From<MintCmd> for Cmd {
    fn from(v: MintCmd) -> Self {
        Self::Mint(v)
    }
}

impl From<StrategyCmd> for Cmd {
    fn from(v: StrategyCmd) -> Self {
        Self::Strategy(v)
    }
}

impl From<UnbondingLogSet> for Cmd {
    fn from(v: UnbondingLogSet) -> Self {
        Self::UnbondingLog(v)
    }
}

#[cfg(test)]
mod test {
    use std::collections::{BTreeMap, HashMap};

    use test_utils::prelude::*;

    use num::{FixedU256, U256};

    use super::*;

    #[derive(Debug, Default, Clone)]
    struct RecipientBatchMetadata {
        next_batch: Option<BatchId>,
        unbonding: DepositValue,
    }

    #[derive(Debug, Default, Clone)]
    struct RecipientMetadata {
        first_entered: BatchId,
        last_entered: BatchId,
        last_claimed: Option<BatchId>,
        batches: BTreeMap<BatchId, RecipientBatchMetadata>,
    }

    #[derive(Debug, Default, Clone)]
    struct Batch {
        total_unbonding: DepositValue,
        total_claimable: ClaimAmount,
        hint: Option<Hint>,
        epoch: Option<UnbondEpoch>,
    }

    #[derive(Debug, Clone)]
    struct Context {
        now: Now,
        total_shares_issued: TotalSharesIssued,
        total_deposits: DepositAmount,
        redemption_rate: Rate,
        unbond_ready: bool,
        unbonding_period: u64,
        last_committed_batch: Option<BatchId>,
        batches: BTreeMap<BatchId, Batch>,
        recipients: HashMap<String, RecipientMetadata>,
    }

    fn shares_asset() -> Asset {
        "shares".to_owned().into()
    }

    fn deposit_asset() -> Asset {
        "dAsset".to_owned().into()
    }

    impl SharesMint for Context {
        fn total_shares_issued(&self) -> TotalSharesIssued {
            self.total_shares_issued
        }

        fn shares_asset(&self) -> Asset {
            shares_asset()
        }
    }

    impl Rate {
        fn invert(self) -> Self {
            FixedU256::from_u128(1)
                .checked_div(self.0)
                .map(Self)
                .unwrap()
        }
    }

    impl Strategy for Context {
        fn now(&self) -> Now {
            self.now
        }

        fn deposit_asset(&self) -> Asset {
            deposit_asset()
        }

        fn underlying_asset_decimals(&self) -> Decimals {
            6
        }

        fn total_deposits_value(&self) -> TotalDepositsValue {
            let unbonded_value: DepositValue = self
                .batches
                .values()
                .filter(|b| b.epoch.is_some())
                .map(|b| b.total_unbonding)
                .sum();

            self.redemption_rate
                .apply_u128(self.total_deposits)
                .unwrap()
                - unbonded_value
        }

        fn deposit_value(&self, amount: DepositAmount) -> DepositValue {
            self.redemption_rate.apply_u128(amount).unwrap()
        }

        fn unbond(&self, value: DepositValue) -> UnbondReadyStatus {
            if self.unbond_ready {
                UnbondReadyStatus::Ready {
                    amount: self.redemption_rate.invert().apply_u128(value).unwrap(),
                    epoch: UnbondEpoch {
                        start: self.now,
                        end: self.now + self.unbonding_period,
                    },
                }
            } else {
                UnbondReadyStatus::Later(None)
            }
        }
    }

    impl UnbondingLog for Context {
        fn last_committed_batch_id(&self) -> Option<BatchId> {
            self.last_committed_batch
        }

        fn batch_unbond_value(&self, batch: BatchId) -> Option<DepositValue> {
            self.batches.get(&batch).map(|b| b.total_unbonding)
        }

        fn batch_claimable_amount(&self, batch: BatchId) -> Option<ClaimAmount> {
            self.batches.get(&batch).map(|b| b.total_claimable)
        }

        fn pending_batch_hint(&self, batch: BatchId) -> Option<Hint> {
            self.batches.get(&batch).and_then(|b| b.hint)
        }

        fn committed_batch_epoch(&self, batch: BatchId) -> Option<UnbondEpoch> {
            self.batches.get(&batch).and_then(|b| b.epoch)
        }

        fn first_entered_batch(&self, recipient: &str) -> Option<BatchId> {
            self.recipients.get(recipient).map(|m| m.first_entered)
        }

        fn last_entered_batch(&self, recipient: &str) -> Option<BatchId> {
            self.recipients.get(recipient).map(|m| m.last_entered)
        }

        fn next_entered_batch(&self, recipient: &str, batch: BatchId) -> Option<BatchId> {
            self.recipients
                .get(recipient)
                .and_then(|m| m.batches.get(&batch))
                .and_then(|b| b.next_batch)
        }

        fn last_claimed_batch(&self, recipient: &str) -> Option<BatchId> {
            self.recipients.get(recipient).and_then(|m| m.last_claimed)
        }

        fn unbonded_value_in_batch(&self, recipient: &str, batch: BatchId) -> Option<DepositValue> {
            self.recipients
                .get(recipient)
                .and_then(|m| m.batches.get(&batch))
                .map(|b| b.unbonding)
        }
    }

    impl Context {
        fn handle_cmd(&mut self, cmd: Cmd) {
            match cmd {
                Cmd::Mint(cmd) => match cmd {
                    MintCmd::Mint { amount, .. } => self.total_shares_issued += amount,
                    MintCmd::Burn { amount } => self.total_shares_issued -= amount,
                },
                Cmd::Strategy(cmd) => {
                    if let StrategyCmd::Deposit { amount } = cmd {
                        self.total_deposits += amount
                    }
                }
                Cmd::UnbondingLog(cmd) => match cmd {
                    UnbondingLogSet::LastCommittedBatchId(batch_id) => {
                        self.last_committed_batch = Some(batch_id)
                    }
                    UnbondingLogSet::BatchTotalUnbondValue { batch, value } => {
                        self.batches.entry(batch).or_default().total_unbonding = value;
                    }
                    UnbondingLogSet::BatchClaimableAmount { batch, amount } => {
                        self.batches.entry(batch).or_default().total_claimable = amount;
                    }
                    UnbondingLogSet::BatchHint { batch, hint } => {
                        self.batches.entry(batch).or_default().hint = Some(hint);
                    }
                    UnbondingLogSet::BatchEpoch { batch, epoch } => {
                        self.batches.entry(batch).or_default().epoch = Some(epoch);
                    }
                    UnbondingLogSet::FirstEnteredBatch { recipient, batch } => {
                        self.recipients
                            .entry(recipient.into_string())
                            .or_default()
                            .first_entered = batch;
                    }
                    UnbondingLogSet::LastEnteredBatch { recipient, batch } => {
                        self.recipients
                            .entry(recipient.into_string())
                            .or_default()
                            .first_entered = batch;
                    }
                    UnbondingLogSet::NextEnteredBatch {
                        recipient,
                        previous,
                        next,
                    } => {
                        self.recipients
                            .entry(recipient.into_string())
                            .or_default()
                            .batches
                            .get_mut(&previous)
                            .unwrap()
                            .next_batch = Some(next);
                    }
                    UnbondingLogSet::LastClaimedBatch { recipient, batch } => {
                        self.recipients
                            .entry(recipient.into_string())
                            .or_default()
                            .last_claimed = Some(batch);
                    }
                    UnbondingLogSet::UnbondedValueInBatch {
                        recipient,
                        batch,
                        value,
                    } => {
                        self.recipients
                            .entry(recipient.into_string())
                            .or_default()
                            .batches
                            .entry(batch)
                            .or_default()
                            .unbonding = value;
                    }
                },
            }
        }
    }

    fn prior_deposits() -> Context {
        Context {
            now: 42,
            total_shares_issued: 10u128.pow(18),
            total_deposits: 10u128.pow(6),
            // Inital deposit at 1.0, since increased to 1.5
            redemption_rate: Rate::from_ratio(3, 2).unwrap(),
            unbond_ready: false,
            unbonding_period: 10,
            last_committed_batch: None,
            batches: BTreeMap::new(),
            recipients: HashMap::new(),
        }
    }

    fn no_prior_deposits() -> Context {
        Context {
            now: 0,
            total_shares_issued: 0,
            total_deposits: 0,
            redemption_rate: Rate::from_ratio(1, 1).unwrap(),
            unbond_ready: false,
            unbonding_period: 10,
            last_committed_batch: None,
            batches: BTreeMap::new(),
            recipients: HashMap::new(),
        }
    }

    fn high_decimal_underlying() -> Context {
        Context {
            now: 42,
            total_shares_issued: 10u128.pow(18),
            total_deposits: 10u128.pow(18),
            redemption_rate: Rate::from_ratio(1, 1).unwrap(),
            unbond_ready: false,
            unbonding_period: 10,
            last_committed_batch: None,
            batches: BTreeMap::new(),
            recipients: HashMap::new(),
        }
    }

    fn total_deposit_loss() -> Context {
        Context {
            now: 42,
            total_shares_issued: 10u128.pow(18),
            total_deposits: 10u128.pow(6),
            redemption_rate: Rate(FixedU256::raw(U256::zero())),
            unbond_ready: false,
            unbonding_period: 10,
            last_committed_batch: None,
            batches: BTreeMap::new(),
            recipients: HashMap::new(),
        }
    }

    fn invalid_deposit_asset() -> Asset {
        "invalid_deposit_asset".to_owned().into()
    }

    fn alice() -> Recipient {
        "alice".to_owned().into()
    }

    fn successful_deposit_response(
        ctx: Context,
        amount: DepositAmount,
        recipient: Recipient,
    ) -> DepositResponse {
        let total_deposits_value = ctx.total_deposits_value();
        let deposit_value = ctx.redemption_rate.apply_u128(amount).unwrap();
        let issued_shares = if total_deposits_value > 0 {
            (deposit_value * ctx.total_shares_issued) / total_deposits_value
        } else {
            deposit_value * 10u128.pow(SHARES_DECIMAL_PLACES - ctx.underlying_asset_decimals())
        };

        DepositResponse {
            cmds: cmds![
                StrategyCmd::Deposit { amount },
                MintCmd::Mint {
                    amount: issued_shares,
                    recipient
                }
            ],
            deposit_value,
            issued_shares,
            total_shares_issued: ctx.total_shares_issued + issued_shares,
            total_deposits_value: total_deposits_value + deposit_value,
        }
    }

    #[rstest]
    #[case::non_zero_valid_deposit_asset(
        prior_deposits(),
        deposit_asset(),
        1_000_000,
        alice(),
        Ok(successful_deposit_response(prior_deposits(), 1_000_000, alice()))
    )]
    #[case::first_non_zero_valid_deposit_asset(
        no_prior_deposits(),
        deposit_asset(),
        1_000_000,
        alice(),
        Ok(successful_deposit_response(no_prior_deposits(), 1_000_000, alice()))
    )]
    #[case::non_zero_invalid_deposit_asset(
        prior_deposits(),
        invalid_deposit_asset(),
        1_000_000,
        alice(),
        Err(Error::InvalidDepositAsset)
    )]
    #[case::zero_valid_deposit_asset(
        prior_deposits(),
        deposit_asset(),
        0,
        alice(),
        Err(Error::CannotDepositZero)
    )]
    #[case::very_large_deposit(
        prior_deposits(),
        deposit_asset(),
        // e.g >226 quintillion ETH in wei units (18 decimals)
        226_854_911_280_625_630_000_000_000_000_000_000_000,
        alice(),
        Err(Error::DepositTooLarge)
    )]
    #[case::very_small_deposit(
        high_decimal_underlying(),
        deposit_asset(),
        1,
        alice(),
        Err(Error::DepositTooSmall)
    )]
    #[case::total_deposit_loss(
        total_deposit_loss(),
        deposit_asset(),
        1_000_000,
        alice(),
        Err(Error::CannotDepositInTotalLossState)
    )]
    fn deposit(
        #[case] deposit_ctx: Context,
        #[case] deposit_asset: Asset,
        #[case] deposit_amount: DepositAmount,
        #[case] mint_recipient: Recipient,
        #[case] expected: Result<DepositResponse, Error>,
    ) {
        let ctx = &deposit_ctx;
        assert_eq!(
            vault(ctx, ctx, ctx).deposit(deposit_asset, deposit_amount, mint_recipient),
            expected
        );
    }

    #[rstest]
    #[case::non_zero_valid_deposit_asset(
        prior_deposits(),
        deposit_asset(),
        1_000_000,
        Ok(StrategyCmd::Deposit { amount: 1_000_000 })
    )]
    #[case::first_non_zero_valid_deposit_asset(
        no_prior_deposits(),
        deposit_asset(),
        1_000_000,
        Ok(StrategyCmd::Deposit { amount: 1_000_000 })
    )]
    #[case::non_zero_invalid_deposit_asset(
        prior_deposits(),
        invalid_deposit_asset(),
        1_000_000,
        Err(Error::InvalidDonationAsset)
    )]
    #[case::zero_valid_deposit_asset(
        prior_deposits(),
        deposit_asset(),
        0,
        Err(Error::CannotDonateZero)
    )]
    fn donate(
        #[case] donate_ctx: Context,
        #[case] donate_asset: Asset,
        #[case] donate_amount: DepositAmount,
        #[case] expected: Result<StrategyCmd, Error>,
    ) {
        let ctx = &donate_ctx;
        assert_eq!(
            vault(ctx, ctx, ctx).donate(donate_asset, donate_amount),
            expected
        );
    }

    fn unbond_ready_ctx() -> Context {
        Context {
            now: 42,
            total_shares_issued: 10u128.pow(18),
            total_deposits: 10u128.pow(6),
            // Inital deposit at 1.0, since increased to 1.5
            redemption_rate: Rate::from_ratio(3, 2).unwrap(),
            unbond_ready: true,
            unbonding_period: 10,
            last_committed_batch: None,
            batches: BTreeMap::new(),
            recipients: HashMap::new(),
        }
    }

    fn first_redemption_unbond_ready_success(
        ctx_before: Context,
        shares_amount: SharesAmount,
        recipient: Recipient,
        result: Result<Vec<Cmd>, Error>,
    ) {
        let cmds = result.unwrap();

        let mut ctx_after = ctx_before.clone();

        for cmd in cmds {
            ctx_after.handle_cmd(cmd);
        }

        assert_eq!(
            ctx_after.total_shares_issued,
            ctx_before.total_shares_issued - shares_amount
        );

        assert_eq!(
            ctx_after.last_committed_batch,
            Some(ctx_before.last_committed_batch.map_or(0, |b| b + 1))
        );

        let expected_total_unbonding =
            (shares_amount * ctx_before.total_deposits_value()) / ctx_before.total_shares_issued;

        let expected_total_claimable = ctx_before
            .redemption_rate
            .invert()
            .apply_u128(expected_total_unbonding)
            .unwrap();

        assert!(matches!(
            ctx_after.batches[&ctx_after.last_committed_batch.unwrap()],
            Batch {
                total_unbonding,
                total_claimable,
                epoch,
                ..
            } if total_unbonding == expected_total_unbonding
            && total_claimable == expected_total_claimable
            && epoch.is_some()
        ));

        let just_committed_batch = ctx_after.last_committed_batch.unwrap();

        assert!(matches!(
            ctx_after.recipients.get(recipient.as_str()).unwrap(),
            RecipientMetadata {
                last_entered,
                batches,
                ..
            } if *last_entered == just_committed_batch
            && batches[&just_committed_batch].unbonding == expected_total_unbonding
        ));
    }

    #[rstest]
    #[case::first_redemption_unbond_ready(
        unbond_ready_ctx(),
        shares_asset(),
        unbond_ready_ctx().total_shares_issued / 2,
        alice(),
        first_redemption_unbond_ready_success
    )]
    fn redeem(
        #[case] redeem_ctx: Context,
        #[case] shares_asset: Asset,
        #[case] shares_amount: SharesAmount,
        #[case] recipient: Recipient,
        #[case] check: impl Fn(Context, SharesAmount, Recipient, Result<Vec<Cmd>, Error>),
    ) {
        let ctx = &redeem_ctx;

        let result = vault(ctx, ctx, ctx).redeem(shares_asset, shares_amount, recipient.clone());

        check(redeem_ctx, shares_amount, recipient, result);
    }

    // fn claim(recipient: Recipient, expected: Result<Vec<Cmd>, Error>) {
    //     todo!()
    // }

    // fn start_unbond(expected: Result<Vec<Cmd>, Error>) {
    //     todo!()
    // }
}
