pub mod positions;
pub mod rates;

use crate::{
    admin::AdminRole,
    cmds,
    hub::positions::{deposit_collateral, redeem_reserves},
    mint::{MintCmd, Synthetic, SyntheticAmount},
    vault::{
        DepositAmount, DepositValue, RedemptionRate, SharesAmount, TotalDepositsValue,
        TotalSharesIssued,
    },
    Asset, Decimals, Identifier, Recipient, Sender, UnauthorizedError,
};

use self::positions::{
    add_vault_reserves, advance, claim_amo_shares, claim_treasury_shares, convert_credit, repay,
    self_liquidate, update_cdp, update_vault, withdraw_collateral, Advance, ConvertCreditError,
    LossError, NothingToClaimError, RedeemReservesError, SelfLiquidateError, SelfLiquidation,
    SharesPool, Vault as VaultPosition, WithdrawCollateralError,
};

pub use self::{
    positions::{AmoShares, Cdp, Collateral, Credit, Debt, SumPaymentRatio, TreasuryShares},
    rates::{AdvanceFee, AmoAllocation, CollateralYieldFee, MaxLtv, ReserveYieldFee},
};

pub type VaultId = Identifier;
pub type Proxy = Identifier;
pub type Treasury = Identifier;
pub type Account = Identifier;
pub type Oracle = Identifier;
pub type Amo = Identifier;
pub type VaultShares = Asset;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),

    #[error(transparent)]
    SharesValueLoss(#[from] LossError),

    #[error(transparent)]
    Withdraw(#[from] WithdrawCollateralError),

    #[error(transparent)]
    SelfLiquidate(#[from] SelfLiquidateError),

    #[error(transparent)]
    ConvertCredit(#[from] ConvertCreditError),

    #[error(transparent)]
    Redeem(#[from] RedeemReservesError),

    #[error(transparent)]
    Claim(#[from] NothingToClaimError),

    #[error("invalid deposit asset")]
    InvalidDepositAsset,

    #[error("cannot deposit zero")]
    CannotDepositZero,

    #[error("vault already registered")]
    VaultAlreadyRegistered,

    #[error("vault not registered")]
    VaultNotRegistered,

    #[error("vault not found")]
    VaultNotFound,

    #[error("synthetic not found")]
    SyntheticNotFound,

    #[error("decimals mismatch")]
    DecimalsMismatch,

    #[error("invalid rate")]
    InvalidRate,

    #[error("not enough collateral")]
    NotEnoughCollateral,

    #[error("max ltv exceeded")]
    MaxLtvExceeded,

    #[error("cannot advance zero")]
    CannotAdvanceZero,

    #[error("invalid synthetic asset")]
    InvalidSyntheticAsset,

    #[error("cannot repay zero")]
    CannotRepayZero,

    #[error("nothing to repay")]
    NothingToRepay,

    #[error("cannot withdraw zero")]
    CannotWithdrawZero,

    #[error("cannot convert zero")]
    CannotConvertZero,

    #[error("cannot mint zero")]
    CannotMintZero,

    #[error("cannot redeem zero")]
    CannotRedeemZero,

    #[error("no treasury set")]
    NoTreasurySet,

    #[error("no amo set")]
    NoAmoSet,

    #[error("deposits disabled")]
    DepositsDisabled,

    #[error("advance disabled")]
    AdvanceDisabled,
}

pub trait SyntheticMint {
    fn syntethic_decimals(&self, synthetic: &Synthetic) -> Option<Decimals>;
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// The deposit proxy address to set, if any
    pub deposit: Option<Proxy>,
    /// The advance proxy address to set, if any
    pub advance: Option<Proxy>,
    /// The redeem proxy address to set, if any
    pub redeem: Option<Proxy>,
    /// The mint proxy address to set, if any
    pub mint: Option<Proxy>,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum VaultCmd {
    Register {
        vault: VaultId,
        synthetic: Synthetic,
    },

    SetDepositsEnabled {
        vault: VaultId,
        enabled: bool,
    },

    SetAdvanceEnabled {
        vault: VaultId,
        enabled: bool,
    },

    SetMaxLtv {
        vault: VaultId,
        max_ltv: MaxLtv,
    },

    SetCollateralYieldFee {
        vault: VaultId,
        fee: CollateralYieldFee,
    },

    SetReserveYieldFee {
        vault: VaultId,
        fee: ReserveYieldFee,
    },

    SetAdvanceFeeRecipient {
        vault: VaultId,
        recipient: Recipient,
    },

    SetFixedAdvanceFee {
        vault: VaultId,
        fee: AdvanceFee,
    },

    SetAdvanceFeeOracle {
        vault: VaultId,
        oracle: Oracle,
    },

    SetAmo {
        vault: VaultId,
        amo: Amo,
    },

    SetAmoAllocation {
        vault: VaultId,
        allocation: AmoAllocation,
    },

    SetDepositProxy {
        vault: VaultId,
        proxy: Proxy,
    },

    SetAdvanceProxy {
        vault: VaultId,
        proxy: Proxy,
    },

    SetRedeemProxy {
        vault: VaultId,
        proxy: Proxy,
    },

    SetMintProxy {
        vault: VaultId,
        proxy: Proxy,
    },

    /// Deposit an `amount` of deposit `asset`s into the vault
    /// NOTE: The downstream libary user MUST provide the *same* `recipient` & `reason`
    /// in the associated `Hub::vault_deposit_callback` call.
    Deposit {
        vault: VaultId,
        asset: Asset,
        amount: DepositAmount,
        callback_recipient: Recipient,
        callback_reason: VaultDepositReason,
    },

    /// Redeem an `amount` of vault `shares` on behalf of a `recipient`
    Redeem {
        vault: VaultId,
        shares: VaultShares,
        amount: SharesAmount,
        recipient: Recipient,
    },
}

// TODO: better name
pub trait Vaults {
    /// Returns the decimals used in the underlying asset value (collateral), if the vault exists at all
    fn underlying_asset_decimals(&self, vault: &VaultId) -> Option<Decimals>;

    /// Returns true if the vault has been registered;
    fn is_registered(&self, vault: &VaultId) -> bool;

    /// Returns true if the vault has deposits enabled, otherwise false.
    /// Panics if the vault is not registered.
    fn deposits_enabled(&self, vault: &VaultId) -> bool;

    /// Returns true if the vault has advance enabled, otherwise false.
    /// Panics if the vault is not registered.
    fn advance_enabled(&self, vault: &VaultId) -> bool;

    /// Returns Some(rate) if the rate has been set
    /// Panics if the vault is not registered.
    fn max_ltv(&self, vault: &VaultId) -> Option<MaxLtv>;

    /// Returns Some(rate) if the rate has been set
    /// Panics if the vault is not registered.
    fn collateral_yield_fee(&self, vault: &VaultId) -> Option<CollateralYieldFee>;

    /// Returns Some(rate) if the rate has been set
    /// Panics if the vault is not registered.
    fn reserve_yield_fee(&self, vault: &VaultId) -> Option<ReserveYieldFee>;

    /// Returns Some(rate) if the rate has been set
    /// Panics if the vault is not registered.
    fn fixed_advance_fee(&self, vault: &VaultId) -> Option<AdvanceFee>;

    /// Returns Some(recipient) if one has been set
    /// Panics if the vault is not registered.
    fn advance_fee_recipient(&self, vault: &VaultId) -> Option<Recipient>;

    /// Returns Some(oracle) if one has been set
    /// Panics if the vault is not registered.
    fn advance_fee_oracle(&self, vault: &VaultId) -> Option<Oracle>;

    /// Returns Some(amo) if one has been set
    /// Panics if the vault is not registered.
    fn amo(&self, vault: &VaultId) -> Option<Amo>;

    /// Returns Some(allocation) if one has been set
    /// Panics if the vault is not registered.
    fn amo_allocation(&self, vault: &VaultId) -> Option<AmoAllocation>;

    /// Returns Some(proxy) if one has been set
    /// Panics if the vault is not registered.
    fn deposit_proxy(&self, vault: &VaultId) -> Option<Proxy>;

    /// Returns Some(proxy) if one has been set
    /// Panics if the vault is not registered.
    fn advance_proxy(&self, vault: &VaultId) -> Option<Proxy>;

    /// Returns Some(proxy) if one has been set
    /// Panics if the vault is not registered.
    fn redeem_proxy(&self, vault: &VaultId) -> Option<Proxy>;

    /// Returns Some(proxy) if one has been set
    /// Panics if the vault is not registered.
    fn mint_proxy(&self, vault: &VaultId) -> Option<Proxy>;

    /// Returns the asset that the vault can accept for deposits
    /// Panics if the vault cannot be found
    fn deposit_asset(&self, vault: &VaultId) -> Asset;

    /// Returns the shares asset that a vault issues
    /// Panics if the vault cannot be found
    fn shares_asset(&self, vault: &VaultId) -> Asset;

    /// Returns the synthetic asset associated with the vault, if the vault has been registered
    /// Panics if the vault is not registered.
    fn synthetic_asset(&self, vault: &VaultId) -> Synthetic;

    /// Returns the total shares issued by the vault
    /// Panics if the vault cannot be found
    fn total_shares_issued(&self, vault: &VaultId) -> TotalSharesIssued;

    /// Returns the total value of all vault deposits in terms of the underlying asset (collateral)
    /// Panics if the vault cannot be found
    fn total_deposits_value(&self, vault: &VaultId) -> TotalDepositsValue;
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum BalanceSheetCmd {
    SetTreasury {
        treasury: Treasury,
    },

    SetCollateralShares {
        vault: VaultId,
        shares: SharesAmount,
    },

    SetCollateralBalance {
        vault: VaultId,
        balance: Collateral,
    },

    SetReserveShares {
        vault: VaultId,
        shares: SharesAmount,
    },

    SetReserveBalance {
        vault: VaultId,
        balance: Collateral,
    },

    SetTreasuryShares {
        vault: VaultId,
        shares: TreasuryShares,
    },

    SetAmoShares {
        vault: VaultId,
        shares: AmoShares,
    },

    SetOverallSumPaymentRatio {
        vault: VaultId,
        spr: SumPaymentRatio,
    },

    SetAccountCollateral {
        vault: VaultId,
        account: Account,
        collateral: Collateral,
    },

    SetAccountDebt {
        vault: VaultId,
        account: Account,
        debt: Debt,
    },

    SetAccountCredit {
        vault: VaultId,
        account: Account,
        credit: Credit,
    },

    SetAccountSumPaymentRatio {
        vault: VaultId,
        account: Account,
        spr: SumPaymentRatio,
    },

    /// Send an `amount` of vault `shares` to a `recipient`
    SendShares {
        shares: VaultShares,
        amount: SharesAmount,
        recipient: Recipient,
    },
}

pub trait BalanceSheet {
    fn treasury(&self) -> Option<Treasury>;

    fn collateral_shares(&self, vault: &VaultId) -> Option<SharesAmount>;

    fn collateral_balance(&self, vault: &VaultId) -> Option<Collateral>;

    fn reserve_shares(&self, vault: &VaultId) -> Option<SharesAmount>;

    fn reserve_balance(&self, vault: &VaultId) -> Option<Collateral>;

    fn treasury_shares(&self, vault: &VaultId) -> Option<TreasuryShares>;

    fn amo_shares(&self, vault: &VaultId) -> Option<AmoShares>;

    fn overall_sum_payment_ratio(&self, vault: &VaultId) -> Option<SumPaymentRatio>;

    fn account_collateral(&self, vault: &VaultId, account: &Account) -> Option<Collateral>;

    fn account_debt(&self, vault: &VaultId, account: &Account) -> Option<Debt>;

    fn account_credit(&self, vault: &VaultId, account: &Account) -> Option<Credit>;

    fn account_sum_payment_ratio(
        &self,
        vault: &VaultId,
        account: &Account,
    ) -> Option<SumPaymentRatio>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
#[repr(u8)]
pub enum VaultDepositReason {
    Deposit = 1,
    RepayUnderlying = 2,
    Mint = 3,
}

pub trait AdvanceFeeOracle {
    fn advance_fee(&self, oracle: &Oracle, recipient: &Recipient) -> Option<AdvanceFee>;
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub enum Cmd {
    Mint(MintCmd),
    Vault(VaultCmd),
    BalanceSheet(BalanceSheetCmd),
}

// extend a Vec<Cmd> type to add a builder method to chain adding different commands
trait CmdVecExt {
    fn push_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self;
}

impl CmdVecExt for Vec<Cmd> {
    fn push_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self {
        self.push(cmd.into());
        self
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct PositionResponse {
    pub cmds: Vec<Cmd>,
    pub cdp: Cdp,
}

pub trait ConfigureHub {
    fn register_vault(
        &self,
        role: AdminRole,
        vault: VaultId,
        synthetic: Synthetic,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_treasury(&self, role: AdminRole, treasury: Treasury) -> Result<Vec<Cmd>, Error>;

    fn set_deposit_enabled(
        &self,
        role: AdminRole,
        vault: VaultId,
        enabled: bool,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_advance_enabled(
        &self,
        role: AdminRole,
        vault: VaultId,
        enabled: bool,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_max_ltv(&self, role: AdminRole, vault: VaultId, bps: u32) -> Result<Vec<Cmd>, Error>;

    fn set_collateral_yield_fee(
        &self,
        role: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_reserve_yield_fee(
        &self,
        role: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_advance_fee_recipient(
        &self,
        role: AdminRole,
        vault: VaultId,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_fixed_advance_fee(
        &self,
        role: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_advance_fee_oracle(
        &self,
        role: AdminRole,
        vault: VaultId,
        oracle: Oracle,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_amo(&self, role: AdminRole, vault: VaultId, amo: Amo) -> Result<Vec<Cmd>, Error>;

    fn set_amo_allocation(
        &self,
        role: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error>;

    fn set_proxy_config(
        &self,
        role: AdminRole,
        vault: VaultId,
        config: ProxyConfig,
    ) -> Result<Vec<Cmd>, Error>;
}

pub trait Hub {
    fn evaluate(&self, vault: VaultId, sender: Sender) -> Result<PositionResponse, Error>;

    fn deposit(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn advance(
        &self,
        vault: VaultId,
        sender: Sender,
        advance_amount: Debt,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn repay_underlying(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
    ) -> Result<Vec<Cmd>, Error>;

    fn repay_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
    ) -> Result<PositionResponse, Error>;

    fn withdraw_collateral(
        &self,
        vault: VaultId,
        sender: Sender,
        collateral_amount: Collateral,
    ) -> Result<PositionResponse, Error>;

    fn self_liquidate_position(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error>;

    fn convert_credit(
        &self,
        vault: VaultId,
        sender: Sender,
        credit_amount: Credit,
    ) -> Result<PositionResponse, Error>;

    fn redeem_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn mint_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error>;

    fn vault_deposit_callback(
        &self,
        vault: VaultId,
        recipient: Recipient,
        reason: VaultDepositReason,
        issued_shares: SharesAmount,
        deposit_value: DepositValue,
    ) -> Result<Vec<Cmd>, Error>;

    fn claim_treasury_shares(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error>;

    fn claim_amo_shares(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error>;
}

pub struct ConfigureHubImpl<'a> {
    vaults: &'a dyn Vaults,
    mint: &'a dyn SyntheticMint,
}

pub fn configure<'a>(vaults: &'a dyn Vaults, mint: &'a dyn SyntheticMint) -> ConfigureHubImpl<'a> {
    ConfigureHubImpl { vaults, mint }
}

macro_rules! issue_cmd {
    ($registry:expr, $vault:ident, $cmd:expr) => {{
        if !$registry.is_registered(&$vault) {
            return Err(Error::VaultNotRegistered);
        }

        Ok(cmds![$cmd])
    }};
}

impl<'a> ConfigureHub for ConfigureHubImpl<'a> {
    fn register_vault(
        &self,
        _: AdminRole,
        vault: VaultId,
        synthetic: Synthetic,
    ) -> Result<Vec<Cmd>, Error> {
        if self.vaults.is_registered(&vault) {
            return Err(Error::VaultAlreadyRegistered);
        }

        let underlying_asset_decimals = self
            .vaults
            .underlying_asset_decimals(&vault)
            .ok_or(Error::VaultNotFound)?;

        let synthetic_decimals = self
            .mint
            .syntethic_decimals(&synthetic)
            .ok_or(Error::SyntheticNotFound)?;

        if underlying_asset_decimals != synthetic_decimals {
            return Err(Error::DecimalsMismatch);
        }

        Ok(cmds![VaultCmd::Register { vault, synthetic }])
    }

    fn set_treasury(&self, _: AdminRole, treasury: Treasury) -> Result<Vec<Cmd>, Error> {
        Ok(cmds![BalanceSheetCmd::SetTreasury { treasury }])
    }

    fn set_deposit_enabled(
        &self,
        _: AdminRole,
        vault: VaultId,
        enabled: bool,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetDepositsEnabled { vault, enabled }
        )
    }

    fn set_advance_enabled(
        &self,
        _: AdminRole,
        vault: VaultId,
        enabled: bool,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetAdvanceEnabled { vault, enabled }
        )
    }

    fn set_max_ltv(&self, _: AdminRole, vault: VaultId, bps: u32) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetMaxLtv {
                vault,
                max_ltv: MaxLtv::new(bps).ok_or(Error::InvalidRate)?
            }
        )
    }

    fn set_collateral_yield_fee(
        &self,
        _: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetCollateralYieldFee {
                vault,
                fee: CollateralYieldFee::new(bps).ok_or(Error::InvalidRate)?
            }
        )
    }

    fn set_reserve_yield_fee(
        &self,
        _: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetReserveYieldFee {
                vault,
                fee: ReserveYieldFee::new(bps).ok_or(Error::InvalidRate)?
            }
        )
    }

    fn set_advance_fee_recipient(
        &self,
        _: AdminRole,
        vault: VaultId,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetAdvanceFeeRecipient { vault, recipient }
        )
    }

    fn set_fixed_advance_fee(
        &self,
        _: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetFixedAdvanceFee {
                vault,
                fee: AdvanceFee::new(bps).ok_or(Error::InvalidRate)?
            }
        )
    }

    fn set_advance_fee_oracle(
        &self,
        _: AdminRole,
        vault: VaultId,
        oracle: Oracle,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetAdvanceFeeOracle { vault, oracle }
        )
    }

    fn set_amo(&self, _: AdminRole, vault: VaultId, amo: Amo) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(self.vaults, vault, VaultCmd::SetAmo { vault, amo })
    }

    fn set_amo_allocation(
        &self,
        _: AdminRole,
        vault: VaultId,
        bps: u32,
    ) -> Result<Vec<Cmd>, Error> {
        issue_cmd!(
            self.vaults,
            vault,
            VaultCmd::SetAmoAllocation {
                vault,
                allocation: AmoAllocation::new(bps).ok_or(Error::InvalidRate)?
            }
        )
    }

    fn set_proxy_config(
        &self,
        _: AdminRole,
        vault: VaultId,
        config: ProxyConfig,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        let mut cmds = vec![];

        if let Some(proxy) = config.deposit {
            let vault = vault.clone();
            cmds.push_cmd(VaultCmd::SetDepositProxy { vault, proxy });
        }

        if let Some(proxy) = config.advance {
            let vault = vault.clone();
            cmds.push_cmd(VaultCmd::SetAdvanceProxy { vault, proxy });
        }

        if let Some(proxy) = config.redeem {
            let vault = vault.clone();
            cmds.push_cmd(VaultCmd::SetRedeemProxy { vault, proxy });
        }

        if let Some(proxy) = config.mint {
            cmds.push_cmd(VaultCmd::SetMintProxy { vault, proxy });
        }

        Ok(cmds)
    }
}

pub struct HubImpl<'a> {
    vaults: &'a dyn Vaults,
    balance_sheet: &'a dyn BalanceSheet,
    advance_fee_oracle: &'a dyn AdvanceFeeOracle,
}

pub fn hub<'a>(
    vaults: &'a dyn Vaults,
    balance_sheet: &'a dyn BalanceSheet,
    advance_fee_oracle: &'a dyn AdvanceFeeOracle,
) -> HubImpl<'a> {
    HubImpl {
        vaults,
        balance_sheet,
        advance_fee_oracle,
    }
}

#[derive(Debug, Clone)]
struct Evaluation {
    prev_vault: VaultPosition,
    prev_cdp: Cdp,
    redemption_rate: Option<RedemptionRate>,
    current_vault: VaultPosition,
    current_cdp: Cdp,
}

fn push_update_vault_position_cmds(
    id: &VaultId,
    old: &VaultPosition,
    new: &VaultPosition,
    cmds: &mut Vec<Cmd>,
) {
    if old.collateral_pool.shares != new.collateral_pool.shares {
        cmds.push_cmd(BalanceSheetCmd::SetCollateralShares {
            vault: id.clone(),
            shares: new.collateral_pool.shares,
        });
    }

    if old.collateral_pool.quota != new.collateral_pool.quota {
        cmds.push_cmd(BalanceSheetCmd::SetCollateralBalance {
            vault: id.clone(),
            balance: new.collateral_pool.quota,
        });
    }

    if old.reserve_pool.shares != new.reserve_pool.shares {
        cmds.push_cmd(BalanceSheetCmd::SetReserveShares {
            vault: id.clone(),
            shares: new.reserve_pool.shares,
        });
    }

    if old.reserve_pool.quota != new.reserve_pool.quota {
        cmds.push_cmd(BalanceSheetCmd::SetReserveBalance {
            vault: id.clone(),
            balance: new.reserve_pool.quota,
        });
    }

    if old.treasury_shares != new.treasury_shares {
        cmds.push_cmd(BalanceSheetCmd::SetTreasuryShares {
            vault: id.clone(),
            shares: new.treasury_shares,
        });
    }

    if old.amo_shares != new.amo_shares {
        cmds.push_cmd(BalanceSheetCmd::SetAmoShares {
            vault: id.clone(),
            shares: new.amo_shares,
        });
    }

    if old.spr != new.spr {
        cmds.push_cmd(BalanceSheetCmd::SetOverallSumPaymentRatio {
            vault: id.clone(),
            spr: new.spr,
        });
    }
}

fn push_update_cdp_cmds(
    vault: &VaultId,
    account: &Account,
    old: &Cdp,
    new: &Cdp,
    cmds: &mut Vec<Cmd>,
) {
    if old.collateral != new.collateral {
        cmds.push_cmd(BalanceSheetCmd::SetAccountCollateral {
            vault: vault.clone(),
            account: account.clone(),
            collateral: new.collateral,
        });
    }

    if old.debt != new.debt {
        cmds.push_cmd(BalanceSheetCmd::SetAccountDebt {
            vault: vault.clone(),
            account: account.clone(),
            debt: new.debt,
        });
    }

    if old.credit != new.credit {
        cmds.push_cmd(BalanceSheetCmd::SetAccountCredit {
            vault: vault.clone(),
            account: account.clone(),
            credit: new.credit,
        });
    }

    if old.spr != new.spr {
        cmds.push_cmd(BalanceSheetCmd::SetAccountSumPaymentRatio {
            vault: vault.clone(),
            account: account.clone(),
            spr: new.spr,
        });
    }
}

impl<'a> HubImpl<'a> {
    pub fn current_vault_position(&self, vault: &VaultId) -> VaultPosition {
        VaultPosition {
            collateral_pool: {
                let shares = self
                    .balance_sheet
                    .collateral_shares(vault)
                    .unwrap_or_default();

                let quota = self
                    .balance_sheet
                    .collateral_balance(vault)
                    .unwrap_or_default();

                SharesPool { shares, quota }
            },
            reserve_pool: {
                let shares = self.balance_sheet.reserve_shares(vault).unwrap_or_default();

                let quota = self
                    .balance_sheet
                    .reserve_balance(vault)
                    .unwrap_or_default();

                SharesPool { shares, quota }
            },
            treasury_shares: self
                .balance_sheet
                .treasury_shares(vault)
                .unwrap_or_default(),
            amo_shares: self.balance_sheet.amo_shares(vault).unwrap_or_default(),
            spr: self
                .balance_sheet
                .overall_sum_payment_ratio(vault)
                .unwrap_or(SumPaymentRatio::zero()),
        }
    }

    pub fn current_cdp(&self, vault: &VaultId, account: &Account) -> Cdp {
        let collateral = self
            .balance_sheet
            .account_collateral(vault, account)
            .unwrap_or_default();

        let debt = self
            .balance_sheet
            .account_debt(vault, account)
            .unwrap_or_default();

        let credit = self
            .balance_sheet
            .account_credit(vault, account)
            .unwrap_or_default();

        let spr = self
            .balance_sheet
            .account_sum_payment_ratio(vault, account)
            .unwrap_or(SumPaymentRatio::zero());

        Cdp {
            collateral,
            debt,
            credit,
            spr,
        }
    }

    fn redemption_rate(&self, id: &VaultId) -> Option<RedemptionRate> {
        let total_shares_issued = self.vaults.total_shares_issued(id);
        let total_deposit_value = self.vaults.total_deposits_value(id);

        RedemptionRate::new(total_shares_issued, total_deposit_value)
    }

    fn max_ltv(&self, vault: &VaultId) -> MaxLtv {
        self.vaults.max_ltv(vault).unwrap_or_default()
    }

    fn advance_fee(&self, vault: &VaultId, recipient: &Recipient) -> AdvanceFee {
        // check if a fee oracle is set
        let Some(oracle) = self.vaults.advance_fee_oracle(vault) else {
            // if not, use the fixed fee
            return self.vaults.fixed_advance_fee(vault).unwrap_or_default();
        };

        // request fee for the recipient from the oracle
        self.advance_fee_oracle
            .advance_fee(&oracle, recipient)
            .unwrap_or_default()
    }

    fn _evaluate(&self, vault_id: &VaultId, account: &Account) -> Result<Evaluation, Error> {
        let prev_vault = self.current_vault_position(vault_id);

        let prev_cdp = self.current_cdp(vault_id, account);

        let redemption_rate = self.redemption_rate(vault_id);

        let current_vault = update_vault(
            prev_vault.clone(),
            redemption_rate,
            || self.vaults.amo_allocation(vault_id).unwrap_or_default(),
            || {
                self.vaults
                    .collateral_yield_fee(vault_id)
                    .unwrap_or_default()
            },
            || self.vaults.reserve_yield_fee(vault_id).unwrap_or_default(),
        )?
        .unwrap_or_else(|| prev_vault.clone());

        let current_cdp = update_cdp(&current_vault, prev_cdp.clone());

        Ok(Evaluation {
            prev_vault,
            prev_cdp,
            redemption_rate,
            current_vault,
            current_cdp,
        })
    }
}

impl<'a> Hub for HubImpl<'a> {
    fn evaluate(&self, vault_id: VaultId, sender: Sender) -> Result<PositionResponse, Error> {
        let evaluation = self._evaluate(&vault_id, &sender)?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &evaluation.current_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &evaluation.current_cdp,
            &mut cmds,
        );

        Ok(PositionResponse {
            cmds,
            cdp: evaluation.current_cdp,
        })
    }

    fn deposit(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if !self.vaults.deposits_enabled(&vault) {
            return Err(Error::DepositsDisabled);
        }

        if self
            .vaults
            .deposit_proxy(&vault)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        if deposit_amount == 0 {
            return Err(Error::CannotDepositZero);
        }

        if deposit_asset != self.vaults.deposit_asset(&vault) {
            return Err(Error::InvalidDepositAsset);
        }

        let PositionResponse { mut cmds, .. } = self.evaluate(vault.clone(), recipient.clone())?;

        cmds.push_cmd(VaultCmd::Deposit {
            vault,
            asset: deposit_asset,
            amount: deposit_amount,
            callback_recipient: recipient,
            callback_reason: VaultDepositReason::Deposit,
        });

        Ok(cmds)
    }

    fn advance(
        &self,
        vault_id: VaultId,
        sender: Sender,
        advance_amount: Debt,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        if !self.vaults.advance_enabled(&vault_id) {
            return Err(Error::AdvanceDisabled);
        }

        if advance_amount == 0 {
            return Err(Error::CannotAdvanceZero);
        }

        if self
            .vaults
            .advance_proxy(&vault_id)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        let evaluation = self._evaluate(&vault_id, &recipient)?;

        let advance_fee_recipient = self.vaults.advance_fee_recipient(&vault_id);

        let Advance {
            cdp: current_cdp,
            amount,
            fee,
        } = advance(
            evaluation.current_cdp,
            advance_amount,
            || self.max_ltv(&vault_id),
            || {
                advance_fee_recipient
                    .is_some()
                    .then(|| self.advance_fee(&vault_id, &recipient))
            },
        )
        .ok_or(Error::NotEnoughCollateral)?;

        let synthetic = self.vaults.synthetic_asset(&vault_id);

        let mut cmds = vec![];

        push_update_cdp_cmds(
            &vault_id,
            &recipient,
            &evaluation.prev_cdp,
            &current_cdp,
            &mut cmds,
        );

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &evaluation.current_vault,
            &mut cmds,
        );

        cmds.push_cmd(MintCmd::Mint {
            synthetic: synthetic.clone(),
            amount,
            recipient,
        });

        if let Some((fee, recipient)) = fee.zip(advance_fee_recipient) {
            cmds.push_cmd(MintCmd::Mint {
                synthetic: synthetic.clone(),
                amount: fee,
                recipient,
            });
        }

        Ok(cmds)
    }

    fn repay_underlying(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if deposit_amount == 0 {
            return Err(Error::CannotRepayZero);
        }

        if deposit_asset != self.vaults.deposit_asset(&vault) {
            return Err(Error::InvalidDepositAsset);
        }

        let PositionResponse { mut cmds, cdp } = self.evaluate(vault.clone(), sender.clone())?;

        if cdp.debt == 0 {
            return Err(Error::NothingToRepay);
        }

        cmds.push_cmd(VaultCmd::Deposit {
            vault,
            asset: deposit_asset,
            amount: deposit_amount,
            callback_recipient: sender,
            callback_reason: VaultDepositReason::RepayUnderlying,
        });

        Ok(cmds)
    }

    fn repay_synthetic(
        &self,
        vault_id: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
    ) -> Result<PositionResponse, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        if synthetic_amount == 0 {
            return Err(Error::CannotRepayZero);
        }

        if synthetic_asset != self.vaults.synthetic_asset(&vault_id) {
            return Err(Error::InvalidSyntheticAsset);
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        if evaluation.current_cdp.debt == 0 {
            return Err(Error::NothingToRepay);
        }

        let updated_cdp = repay(evaluation.current_cdp, synthetic_amount);

        let mut cmds = vec![];

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &updated_cdp,
            &mut cmds,
        );

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &evaluation.current_vault,
            &mut cmds,
        );

        cmds.push_cmd(MintCmd::Burn {
            synthetic: synthetic_asset,
            amount: synthetic_amount,
        });

        Ok(PositionResponse {
            cmds,
            cdp: updated_cdp,
        })
    }

    fn withdraw_collateral(
        &self,
        vault_id: VaultId,
        sender: Sender,
        collateral_amount: Collateral,
    ) -> Result<PositionResponse, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        if collateral_amount == 0 {
            return Err(Error::CannotWithdrawZero);
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let max_ltv = self.max_ltv(&vault_id);

        let (updated_vault, updated_cdp, shares_amount) = withdraw_collateral(
            evaluation.current_vault,
            evaluation.current_cdp,
            collateral_amount,
            max_ltv,
            evaluation.redemption_rate,
        )?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &updated_cdp,
            &mut cmds,
        );

        let shares_asset = self.vaults.shares_asset(&vault_id);

        cmds.push_cmd(VaultCmd::Redeem {
            vault: vault_id,
            shares: shares_asset,
            amount: shares_amount,
            recipient: sender,
        });

        Ok(PositionResponse {
            cmds,
            cdp: updated_cdp,
        })
    }

    fn self_liquidate_position(
        &self,
        vault_id: VaultId,
        sender: Sender,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let SelfLiquidation {
            vault: updated_vault,
            cdp: updated_cdp,
            mint_credit,
            redeem_shares,
        } = self_liquidate(
            evaluation.current_vault,
            evaluation.current_cdp,
            evaluation.redemption_rate,
        )?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &updated_cdp,
            &mut cmds,
        );

        if let Some(amount) = mint_credit {
            let synthetic = self.vaults.synthetic_asset(&vault_id);

            cmds.push_cmd(MintCmd::Mint {
                synthetic,
                amount,
                recipient: sender.clone(),
            });
        }

        if let Some(amount) = redeem_shares {
            let shares_asset = self.vaults.shares_asset(&vault_id);

            cmds.push_cmd(VaultCmd::Redeem {
                vault: vault_id,
                shares: shares_asset,
                amount,
                recipient: sender,
            });
        }

        Ok(cmds)
    }

    fn convert_credit(
        &self,
        vault_id: VaultId,
        sender: Sender,
        credit_amount: Credit,
    ) -> Result<PositionResponse, Error> {
        if credit_amount == 0 {
            return Err(Error::CannotConvertZero);
        }

        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let (updated_vault, updated_cdp) = convert_credit(
            evaluation.current_vault,
            evaluation.current_cdp,
            credit_amount,
            evaluation.redemption_rate,
        )?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &updated_cdp,
            &mut cmds,
        );

        Ok(PositionResponse {
            cmds,
            cdp: updated_cdp,
        })
    }

    fn redeem_synthetic(
        &self,
        vault_id: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if synthetic_amount == 0 {
            return Err(Error::CannotRedeemZero);
        }

        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        if synthetic_asset != self.vaults.synthetic_asset(&vault_id) {
            return Err(Error::InvalidSyntheticAsset);
        }

        if self
            .vaults
            .redeem_proxy(&vault_id)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let (updated_vault, shares_amount) = redeem_reserves(
            evaluation.current_vault,
            synthetic_amount,
            evaluation.redemption_rate,
        )?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &recipient,
            &evaluation.prev_cdp,
            &evaluation.current_cdp,
            &mut cmds,
        );

        let shares_asset = self.vaults.shares_asset(&vault_id);

        cmds.push_cmd(VaultCmd::Redeem {
            vault: vault_id,
            shares: shares_asset,
            amount: shares_amount,
            recipient,
        })
        .push_cmd(MintCmd::Burn {
            synthetic: synthetic_asset,
            amount: synthetic_amount,
        });

        Ok(cmds)
    }

    fn mint_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        deposit_asset: Asset,
        deposit_amount: DepositAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if deposit_amount == 0 {
            return Err(Error::CannotMintZero);
        }

        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if !self.vaults.deposits_enabled(&vault) {
            return Err(Error::DepositsDisabled);
        }

        if self
            .vaults
            .mint_proxy(&vault)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        if deposit_asset != self.vaults.deposit_asset(&vault) {
            return Err(Error::InvalidDepositAsset);
        }

        let PositionResponse { mut cmds, .. } = self.evaluate(vault.clone(), recipient.clone())?;

        cmds.push_cmd(VaultCmd::Deposit {
            vault,
            asset: deposit_asset,
            amount: deposit_amount,
            callback_recipient: recipient,
            callback_reason: VaultDepositReason::Mint,
        });

        Ok(cmds)
    }

    fn vault_deposit_callback(
        &self,
        vault_id: VaultId,
        recipient: Recipient,
        reason: VaultDepositReason,
        issued_shares: SharesAmount,
        deposit_value: DepositValue,
    ) -> Result<Vec<Cmd>, Error> {
        assert!(self.vaults.is_registered(&vault_id));

        let mut cmds = vec![];

        match reason {
            VaultDepositReason::Deposit => {
                let current_vault = self.current_vault_position(&vault_id);

                let current_cdp = self.current_cdp(&vault_id, &recipient);

                let (updated_vault, updated_cdp) = deposit_collateral(
                    current_vault.clone(),
                    current_cdp.clone(),
                    deposit_value,
                    issued_shares,
                );

                push_update_vault_position_cmds(
                    &vault_id,
                    &current_vault,
                    &updated_vault,
                    &mut cmds,
                );

                push_update_cdp_cmds(&vault_id, &recipient, &current_cdp, &updated_cdp, &mut cmds);
            }

            VaultDepositReason::RepayUnderlying => {
                let current_vault = self.current_vault_position(&vault_id);

                let current_cdp = self.current_cdp(&vault_id, &recipient);

                let updated_vault =
                    add_vault_reserves(current_vault.clone(), deposit_value, issued_shares);

                let updated_cdp = repay(current_cdp.clone(), deposit_value);

                push_update_vault_position_cmds(
                    &vault_id,
                    &current_vault,
                    &updated_vault,
                    &mut cmds,
                );

                push_update_cdp_cmds(&vault_id, &recipient, &current_cdp, &updated_cdp, &mut cmds);
            }

            VaultDepositReason::Mint => {
                let current_vault = self.current_vault_position(&vault_id);

                let updated_vault =
                    add_vault_reserves(current_vault.clone(), deposit_value, issued_shares);

                push_update_vault_position_cmds(
                    &vault_id,
                    &current_vault,
                    &updated_vault,
                    &mut cmds,
                );

                let synthetic = self.vaults.synthetic_asset(&vault_id);

                cmds.push_cmd(MintCmd::Mint {
                    synthetic,
                    amount: deposit_value,
                    recipient,
                });
            }
        };

        Ok(cmds)
    }

    fn claim_treasury_shares(&self, vault_id: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        let treasury = self.balance_sheet.treasury().ok_or(Error::NoTreasurySet)?;

        if sender != treasury {
            return Err(UnauthorizedError.into());
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let (updated_vault, treasury_shares) = claim_treasury_shares(evaluation.current_vault)?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &evaluation.current_cdp,
            &mut cmds,
        );

        let shares_asset = self.vaults.shares_asset(&vault_id);

        cmds.push_cmd(BalanceSheetCmd::SendShares {
            shares: shares_asset,
            amount: treasury_shares,
            recipient: treasury,
        });

        Ok(cmds)
    }

    fn claim_amo_shares(&self, vault_id: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault_id) {
            return Err(Error::VaultNotRegistered);
        }

        let amo = self.vaults.amo(&vault_id).ok_or(Error::NoAmoSet)?;

        if sender != amo {
            return Err(UnauthorizedError.into());
        }

        let evaluation = self._evaluate(&vault_id, &sender)?;

        let (updated_vault, amo_shares) = claim_amo_shares(evaluation.current_vault)?;

        let mut cmds = vec![];

        push_update_vault_position_cmds(
            &vault_id,
            &evaluation.prev_vault,
            &updated_vault,
            &mut cmds,
        );

        push_update_cdp_cmds(
            &vault_id,
            &sender,
            &evaluation.prev_cdp,
            &evaluation.current_cdp,
            &mut cmds,
        );

        let shares_asset = self.vaults.shares_asset(&vault_id);

        cmds.push_cmd(BalanceSheetCmd::SendShares {
            shares: shares_asset,
            amount: amo_shares,
            recipient: amo,
        });

        Ok(cmds)
    }
}

impl From<MintCmd> for Cmd {
    fn from(v: MintCmd) -> Self {
        Self::Mint(v)
    }
}

impl From<VaultCmd> for Cmd {
    fn from(v: VaultCmd) -> Self {
        Self::Vault(v)
    }
}

impl From<BalanceSheetCmd> for Cmd {
    fn from(v: BalanceSheetCmd) -> Self {
        Self::BalanceSheet(v)
    }
}

#[cfg(test)]
mod test;
