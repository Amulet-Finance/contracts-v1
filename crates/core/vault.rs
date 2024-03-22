use crate::{Asset, Decimals, Rate, Recipient, cmds};

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

pub struct UnbondEpoch {
    pub start: Instant,
    pub end: Instant,
}

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

pub trait Mint {
    /// Returns the total issued shares
    fn total_shares_issued(&self) -> TotalSharesIssued;

    /// Returns asset that the strategy issues as shares
    fn shares_asset(&self) -> Asset;
}

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

    pub fn checked_deposits_to_shares(
        &self,
        deposit_amount: DepositAmount,
    ) -> Option<SharesAmount> {
        Rate::from_ratio(deposit_amount, self.total_deposits_value)
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
        self.checked_shares_to_deposits(deposit_amount)
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

#[derive(Debug, thiserror::Error)]
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
    mint: &'a dyn Mint,
}

pub fn vault<'a>(
    strategy: &'a dyn Strategy,
    unbonding_log: &'a dyn UnbondingLog,
    mint: &'a dyn Mint,
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

        let next_id = match self.unbonding_log.last_claimed_batch(self.recipient) {
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

        self.try_batch(highest_id, next_id)
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

        // Deposit the assets, getting their actual value in terms of the underlying strategy token
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
            deposit_value,
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
