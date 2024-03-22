use crate::{
    admin::AdminRole,
    cmds,
    mint::{MintCmd, Synthetic, SyntheticAmount},
    num::{FixedU256, U256},
    vault::{
        DepositAmount, DepositValue, RedemptionRate, SharesAmount, TotalDepositsValue,
        TotalSharesIssued,
    },
    Asset, Decimals, Identifier, Rate, Recipient, Sender, UnauthorizedError,
};

pub type VaultId = Identifier;
pub type Proxy = Identifier;
pub type Treasury = Identifier;
pub type Account = Identifier;
pub type Oracle = Identifier;
pub type Amo = Identifier;
pub type VaultShares = Asset;
pub type CollateralPoolShares = SharesAmount;
pub type ReservePoolShares = SharesAmount;
pub type TreasuryShares = SharesAmount;
pub type AmoShares = SharesAmount;
pub type Collateral = u128;
pub type Debt = u128;
pub type Credit = u128;
pub type FeeAmount = u128;

macro_rules! bps_rate {
    ($T:ident, max=$max:expr, default=$default:expr) => {
        pub struct $T {
            bps: u32,
            rate: Rate,
        }

        impl $T {
            pub const MAX: u32 = $max;

            pub fn new(bps: u32) -> Option<Self> {
                if bps > Self::MAX {
                    return None;
                }

                let rate = Rate::from_ratio(bps.into(), 10_000).unwrap();

                Some(Self { bps, rate })
            }

            pub fn raw(self) -> u32 {
                self.bps
            }

            fn rate(self) -> Rate {
                self.rate
            }
        }

        ::static_assertions::const_assert!($default <= $max);

        impl Default for $T {
            fn default() -> Self {
                Self::new($default).unwrap()
            }
        }
    };
}

macro_rules! percent {
    ($x:literal) => {
        $x * 100
    };
}

bps_rate!(MaxLtv, max = percent!(100), default = percent!(50));

bps_rate!(
    CollateralYieldFee,
    max = percent!(100),
    default = percent!(10)
);

bps_rate!(
    ReserveYieldFee,
    max = percent!(100),
    default = percent!(100)
);

bps_rate!(
    AdvanceFee,
    max = percent!(50),
    default = 25 // bps: 0.25%
);

bps_rate!(
    AmoAllocation,
    max = percent!(100),
    default = 0 // bps: 0.0%
);

/// Σ x/y - where x is a debt payment and y is the collateral balance at the time of payment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SumPaymentRatio(FixedU256);

impl SumPaymentRatio {
    pub fn raw(x: U256) -> Self {
        Self(FixedU256::raw(x))
    }

    const fn zero() -> SumPaymentRatio {
        Self(FixedU256::raw(U256::zero()))
    }

    pub fn into_raw(self) -> U256 {
        self.0.into_raw()
    }

    pub fn add_ratio(self, payment: Debt, collateral: Collateral) -> Self {
        assert!(collateral > 0, "collateral cannot be zero");
        assert!(payment > 0, "payment cannot be zero");

        let ratio = FixedU256::from_u128(payment)
            .checked_div(FixedU256::from_u128(collateral))
            .expect("collateral > 0");

        self.0
            .checked_add(ratio)
            .map(Self)
            .expect("sum payment ratio should never overflow")
    }

    pub fn abs_diff(self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }

    pub fn debt_payment(self, collateral: Collateral) -> Debt {
        self.0
            .checked_mul(FixedU256::from_u128(collateral))
            .map(FixedU256::floor)
            .expect("earnings should never overflow")
    }
}

pub trait SyntheticMint {
    fn syntethic_decimals(&self, synthetic: &Synthetic) -> Option<Decimals>;
}

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

#[repr(u8)]
pub enum VaultDepositReason {
    Deposit = 1,
    RepayUnderlying = 2,
    Mint = 3,
}

pub trait AdvanceFeeOracle {
    fn advance_fee(&self, oracle: &Oracle, recipient: &Recipient) -> Option<AdvanceFee>;
}

pub enum Cmd {
    Mint(MintCmd),
    Vault(VaultCmd),
    BalanceSheet(BalanceSheetCmd),
}

// extend a Vec<Cmd> type to add a builder method to chain adding different commands
trait CmdVecExt {
    fn push_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self;

    fn add_cmd(mut self, cmd: impl Into<Cmd>) -> Self
    where
        Self: Sized,
    {
        self.push_cmd(cmd);
        self
    }
}

impl CmdVecExt for Vec<Cmd> {
    fn push_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self {
        self.push(cmd.into());
        self
    }
}

#[derive(Debug, thiserror::Error)]
#[error("shares value loss")]
pub struct SharesValueLossError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),

    #[error(transparent)]
    SharesValueLoss(#[from] SharesValueLossError),

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

    #[error("cannot withdraw zero")]
    CannotWithdrawZero,

    #[error("nothing to liquidate")]
    NothingToLiquidate,

    #[error("not enough credit")]
    NotEnoughCredit,

    #[error("cannot convert zero")]
    CannotConvertZero,

    #[error("insufficient reserve balance")]
    InsufficientReserveBalance,

    #[error("cannot mint zero")]
    CannotMintZero,

    #[error("no treasury set")]
    NoTreasurySet,

    #[error("nothing to claim")]
    NothingToClaim,

    #[error("no amo set")]
    NoAmoSet,
}

#[derive(Debug, Clone, Copy)]
pub struct Cdp {
    pub collateral: Collateral,
    pub debt: Debt,
    pub credit: Credit,
    pub spr: SumPaymentRatio,
}

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

#[derive(Debug, Clone, Copy)]
struct SharesPool {
    shares: SharesAmount,
    quota: Collateral,
}

#[derive(Debug, Clone, Copy)]
struct Surplus {
    shares: SharesAmount,
    redemption_rate: RedemptionRate,
}

#[derive(Debug, Clone, Copy)]
struct TreasuryPayment {
    shares: SharesAmount,
}

#[derive(Debug, Clone, Copy)]
struct OverallDebtPayment {
    shares: SharesAmount,
    value: Debt,
    redemption_rate: RedemptionRate,
}

#[derive(Debug, Clone, Copy)]
struct AmoPayment {
    shares: SharesAmount,
}

#[derive(Debug, Clone, Copy)]
struct ReservePoolPayment {
    shares: SharesAmount,
    value: Debt,
}

#[derive(Debug, Clone, Copy)]
struct Payments {
    treasury: TreasuryPayment,
    overall_debt: OverallDebtPayment,
    reserve_pool: ReservePoolPayment,
    amo: AmoPayment,
}

type CollateralPool = SharesPool;
type ReservePool = SharesPool;
type CollateralPoolSurplus = Surplus;
type ReservePoolSurplus = Surplus;

#[derive(Debug, Clone, Copy)]
struct BalanceSheetState {
    collateral_pool: CollateralPool,
    reserve_pool: ReservePool,
    treasury_shares: TreasuryShares,
    amo_shares: AmoShares,
    spr: SumPaymentRatio,
}

struct BalanceSheetDiff {
    old: BalanceSheetState,
    new: BalanceSheetState,
    redemption_rate: RedemptionRate,
}

impl BalanceSheetDiff {
    fn balance_sheet(&self) -> BalanceSheetState {
        self.new
    }

    fn redemption_rate(&self) -> RedemptionRate {
        self.redemption_rate
    }

    fn into_cmds(self, vault: &VaultId) -> Vec<Cmd> {
        let mut cmds = vec![];

        if self.old.collateral_pool.shares != self.new.collateral_pool.shares {
            cmds.push_cmd(BalanceSheetCmd::SetCollateralShares {
                vault: vault.clone(),
                shares: self.new.collateral_pool.shares,
            });
        }

        if self.old.reserve_pool.shares != self.new.reserve_pool.shares {
            cmds.push_cmd(BalanceSheetCmd::SetReserveShares {
                vault: vault.clone(),
                shares: self.new.reserve_pool.shares,
            });
        }

        if self.old.reserve_pool.quota != self.new.reserve_pool.quota {
            cmds.push_cmd(BalanceSheetCmd::SetReserveBalance {
                vault: vault.clone(),
                balance: self.new.reserve_pool.quota,
            });
        }

        if self.old.treasury_shares != self.new.treasury_shares {
            cmds.push_cmd(BalanceSheetCmd::SetTreasuryShares {
                vault: vault.clone(),
                shares: self.new.treasury_shares,
            });
        }

        if self.old.amo_shares != self.new.amo_shares {
            cmds.push_cmd(BalanceSheetCmd::SetAmoShares {
                vault: vault.clone(),
                shares: self.new.amo_shares,
            });
        }

        if self.old.amo_shares != self.new.amo_shares {
            cmds.push_cmd(BalanceSheetCmd::SetAmoShares {
                vault: vault.clone(),
                shares: self.new.amo_shares,
            });
        }

        if self.old.spr != self.new.spr {
            cmds.push_cmd(BalanceSheetCmd::SetOverallSumPaymentRatio {
                vault: vault.clone(),
                spr: self.new.spr,
            });
        }

        cmds
    }
}

macro_rules! safe_add {
    ($lhs:expr, $rhs:expr) => {
        $lhs.checked_add($rhs)
            .unwrap_or_else(|| panic!("overflow adding {} to {}", $lhs, $rhs))
    };
}

impl Payments {
    fn combine_with(self, other: Self) -> Self {
        Self {
            treasury: TreasuryPayment {
                shares: safe_add!(self.treasury.shares, other.treasury.shares),
            },
            overall_debt: OverallDebtPayment {
                shares: safe_add!(self.overall_debt.shares, other.overall_debt.shares),
                value: safe_add!(self.overall_debt.value, other.overall_debt.value),
                ..self.overall_debt
            },
            reserve_pool: ReservePoolPayment {
                shares: safe_add!(self.reserve_pool.shares, other.reserve_pool.shares),
                value: safe_add!(self.reserve_pool.value, other.reserve_pool.value),
            },
            amo: AmoPayment {
                shares: safe_add!(self.amo.shares, other.amo.shares),
            },
        }
    }
}

impl SharesPool {
    fn surplus(
        &self,
        redemption_rate: RedemptionRate,
    ) -> Result<Option<Surplus>, SharesValueLossError> {
        let pool_shares_value = redemption_rate.shares_to_deposits(self.shares);

        if pool_shares_value < self.quota {
            return Err(SharesValueLossError);
        }

        if pool_shares_value == self.quota {
            return Ok(None);
        }

        let surplus_value = self
            .quota
            .checked_sub(pool_shares_value)
            .expect("quota >= pool shares value");

        let surplus_shares = redemption_rate.deposits_to_shares(surplus_value);

        Ok(Some(Surplus {
            shares: surplus_shares,
            redemption_rate,
        }))
    }

    fn add(self, shares_increase: SharesAmount, quota_increase: Collateral) -> Self {
        let shares = safe_add!(self.shares, shares_increase);

        let quota = safe_add!(self.quota, quota_increase);

        Self { shares, quota }
    }

    fn remove(self, shares_decrease: SharesAmount, quota_decrease: Collateral) -> Option<Self> {
        let shares = self.shares.checked_sub(shares_decrease)?;

        let quota = self.quota.checked_sub(quota_decrease)?;

        Some(Self { shares, quota })
    }
}

impl Surplus {
    fn apply_fee(&self, fee: Rate) -> (TreasuryPayment, OverallDebtPayment) {
        let treasury_payment_shares = fee.apply_u128(self.shares).expect("fee <= 100%");

        let debt_payment_shares = self
            .shares
            .checked_sub(treasury_payment_shares)
            .expect("fee <= 100%");

        let debt_payment_value = self.redemption_rate.shares_to_deposits(debt_payment_shares);

        (
            TreasuryPayment {
                shares: treasury_payment_shares,
            },
            OverallDebtPayment {
                shares: debt_payment_shares,
                value: debt_payment_value,
                redemption_rate: self.redemption_rate,
            },
        )
    }

    fn payments(&self, treasury_fee: Rate, amo_split: Rate) -> Payments {
        let (treasury, overall_debt) = self.apply_fee(treasury_fee);
        let (reserve_pool, amo) = overall_debt.reserve_amo_split(amo_split);

        Payments {
            treasury,
            overall_debt,
            reserve_pool,
            amo,
        }
    }
}

impl OverallDebtPayment {
    fn reserve_amo_split(&self, split: Rate) -> (ReservePoolPayment, AmoPayment) {
        let amo_allocation_shares = split.apply_u128(self.shares).expect("split <= 100%");

        let reserve_allocation_shares = self
            .shares
            .checked_sub(amo_allocation_shares)
            .expect("fee <= 100%");

        let reserve_allocation_value = self
            .redemption_rate
            .shares_to_deposits(reserve_allocation_shares);

        (
            ReservePoolPayment {
                shares: reserve_allocation_shares,
                value: reserve_allocation_value,
            },
            AmoPayment {
                shares: amo_allocation_shares,
            },
        )
    }
}

impl BalanceSheetState {
    fn update(
        self,
        collateral_pool_surplus: Option<CollateralPoolSurplus>,
        reserve_pool_surplus: Option<ReservePoolSurplus>,
        payments: Payments,
    ) -> Self {
        macro_rules! safe_sub_surplus {
            ($shares_balance:expr, $surplus:expr) => {
                $shares_balance
                    .checked_add($surplus)
                    .expect("surplus <= total")
            };
        }

        match (collateral_pool_surplus, reserve_pool_surplus) {
            // No deposits but some reserve earning yield - it all goes to the treasury and/or AMO
            (None, Some(reserve_surplus)) => {
                let reserve_pool = SharesPool {
                    shares: safe_sub_surplus!(self.reserve_pool.shares, reserve_surplus.shares),
                    ..self.reserve_pool
                };

                let treasury_shares = safe_add!(self.treasury_shares, payments.treasury.shares);
                let treasury_shares = safe_add!(treasury_shares, payments.reserve_pool.shares);

                let amo_shares = safe_add!(self.amo_shares, payments.amo.shares);

                BalanceSheetState {
                    reserve_pool,
                    treasury_shares,
                    amo_shares,
                    spr: SumPaymentRatio::zero(),
                    ..self
                }
            }

            // No reserve yet, lets create one and/or an AMO allocation
            (Some(collateral_surplus), None) => {
                let collateral_pool = SharesPool {
                    shares: safe_sub_surplus!(
                        self.collateral_pool.shares,
                        collateral_surplus.shares
                    ),
                    ..self.collateral_pool
                };

                let reserve_pool = SharesPool {
                    shares: safe_add!(self.reserve_pool.shares, payments.reserve_pool.shares),
                    quota: safe_add!(self.reserve_pool.quota, payments.reserve_pool.value),
                };

                let treasury_shares = safe_add!(self.treasury_shares, payments.treasury.shares);

                let amo_shares = safe_add!(self.amo_shares, payments.amo.shares);

                let spr = if payments.overall_debt.value > 0 && self.collateral_pool.quota > 0 {
                    self.spr
                        .add_ratio(payments.overall_debt.value, self.collateral_pool.quota)
                } else {
                    self.spr
                };

                BalanceSheetState {
                    collateral_pool,
                    reserve_pool,
                    treasury_shares,
                    amo_shares,
                    spr,
                }
            }

            // Yield earned on both collateral & reserve, most likely path
            (Some(collateral_surplus), Some(reserve_surplus)) => {
                let collateral_pool = SharesPool {
                    shares: safe_sub_surplus!(
                        self.collateral_pool.shares,
                        collateral_surplus.shares
                    ),
                    ..self.collateral_pool
                };

                let reserve_pool = SharesPool {
                    shares: safe_sub_surplus!(
                        safe_add!(self.reserve_pool.shares, payments.reserve_pool.shares),
                        reserve_surplus.shares
                    ),
                    quota: safe_add!(self.reserve_pool.quota, payments.reserve_pool.value),
                };

                let treasury_shares = safe_add!(self.treasury_shares, payments.treasury.shares);

                let amo_shares = safe_add!(self.amo_shares, payments.amo.shares);

                let spr = if payments.overall_debt.value > 0 && self.collateral_pool.quota > 0 {
                    self.spr
                        .add_ratio(payments.overall_debt.value, self.collateral_pool.quota)
                } else {
                    self.spr
                };

                BalanceSheetState {
                    collateral_pool,
                    reserve_pool,
                    treasury_shares,
                    amo_shares,
                    spr,
                }
            }

            (None, None) => {
                unreachable!("at least one surplus required for there to be some payments")
            }
        }
    }

    fn withdraw_collateral(
        self,
        redemption_rate: RedemptionRate,
        withdrawal_amount: Collateral,
    ) -> Option<(Self, SharesAmount)> {
        let shares_amount = redemption_rate.deposits_to_shares(withdrawal_amount);

        let collateral_pool = self
            .collateral_pool
            .remove(shares_amount, withdrawal_amount)?;

        Some((
            Self {
                collateral_pool,
                ..self
            },
            shares_amount,
        ))
    }

    fn withdraw_reserve(
        self,
        redemption_rate: RedemptionRate,
        withdrawal_amount: Collateral,
    ) -> Option<(Self, SharesAmount)> {
        let shares_amount = redemption_rate.deposits_to_shares(withdrawal_amount);

        let reserve_pool = self.reserve_pool.remove(shares_amount, withdrawal_amount)?;

        Some((
            Self {
                reserve_pool,
                ..self
            },
            shares_amount,
        ))
    }

    fn move_collateral_to_reserve(
        self,
        redemption_rate: RedemptionRate,
        collateral: Collateral,
    ) -> Option<Self> {
        let shares_amount = redemption_rate.deposits_to_shares(collateral);

        let collateral_pool = self.collateral_pool.remove(shares_amount, collateral)?;

        let reserve_pool = self.reserve_pool.add(shares_amount, collateral);

        Some(Self {
            collateral_pool,
            reserve_pool,
            ..self
        })
    }

    fn move_reserves_to_collateral(
        self,
        redemption_rate: RedemptionRate,
        collateral: Collateral,
    ) -> Option<Self> {
        let shares_amount = redemption_rate.deposits_to_shares(collateral);

        let reserve_pool = self.reserve_pool.remove(shares_amount, collateral)?;

        let collateral_pool = self.collateral_pool.add(shares_amount, collateral);

        Some(Self {
            collateral_pool,
            reserve_pool,
            ..self
        })
    }
}

struct DebtIncreaseRequired {
    cdp: Cdp,
    amount: Debt,
}

struct IssueDebt {
    cdp: Cdp,
    amount: Debt,
    fee: Option<FeeAmount>,
}

enum AdvanceKind {
    CoveredByCredit(Cdp),
    DebtIncreaseRequired(DebtIncreaseRequired),
}

struct SelfLiquidation {
    credit: Option<Credit>,
    withdrawal: Option<Collateral>,
    repayment: Option<Debt>,
}

impl Cdp {
    const fn zero() -> Self {
        Self {
            collateral: 0,
            debt: 0,
            credit: 0,
            spr: SumPaymentRatio::zero(),
        }
    }

    fn update(mut self, overall_spr: SumPaymentRatio) -> Self {
        let debt_payment = overall_spr.abs_diff(self.spr).debt_payment(self.collateral);

        self.spr = overall_spr;

        if debt_payment == 0 {
            return self;
        }

        if self.debt == 0 {
            self.credit = safe_add!(self.credit, debt_payment);

            return self;
        }

        let debt_diff = debt_payment.abs_diff(self.debt);

        self.debt = self.debt.saturating_sub(debt_payment);

        if self.debt > 0 {
            return self;
        }

        self.credit = safe_add!(self.credit, debt_diff);

        self
    }

    fn advance(self, amount: Debt) -> Option<AdvanceKind> {
        if self.credit >= amount {
            let credit = self
                .credit
                .checked_sub(amount)
                .expect("checked: credit >= amount");

            return Some(AdvanceKind::CoveredByCredit(Self { credit, ..self }));
        }

        if self.collateral == 0 {
            return None;
        }

        let debt_increase = DebtIncreaseRequired {
            cdp: Cdp { credit: 0, ..self },
            amount: self.credit.abs_diff(amount),
        };

        Some(AdvanceKind::DebtIncreaseRequired(debt_increase))
    }

    fn repay(self, amount: Debt) -> Cdp {
        if self.debt == 0 {
            let credit = safe_add!(self.credit, amount);

            return Cdp { credit, ..self };
        }

        if amount <= self.debt {
            let debt = self.debt.saturating_sub(amount);

            return Cdp { debt, ..self };
        }

        let credit_increase = self.debt.abs_diff(amount);

        let credit = safe_add!(self.credit, credit_increase);

        Cdp {
            credit,
            debt: 0,
            ..self
        }
    }

    fn withdraw(self, max_ltv: MaxLtv, amount: Collateral) -> Option<Self> {
        if amount > self.collateral {
            return None;
        }

        let collateral = self
            .collateral
            .checked_sub(amount)
            .expect("checked: amount <= collateral balance");

        if self.debt == 0 {
            return Some(Self { collateral, ..self });
        }

        if collateral == 0 {
            return None;
        }

        let proposed_ltv =
            Rate::from_ratio(self.debt, collateral).expect("checked: collateral > 0");

        if proposed_ltv > max_ltv.rate() {
            return None;
        }

        Some(Cdp { collateral, ..self })
    }

    fn self_liquidate(self) -> Option<(Self, SelfLiquidation)> {
        if self.collateral == 0 && self.debt == 0 && self.credit == 0 {
            return None;
        }

        let withdrawal = if self.debt > 0 {
            self.collateral
                .checked_sub(self.debt)
                .expect("always: collateral >= debt")
        } else {
            self.collateral
        };

        let self_liquidation = SelfLiquidation {
            credit: (self.credit != 0).then_some(self.credit),
            withdrawal: (withdrawal != 0).then_some(withdrawal),
            repayment: (self.debt != 0).then_some(self.debt),
        };

        Some((Self::zero(), self_liquidation))
    }

    fn convert_credit(self, credit_amount: u128) -> Option<Self> {
        let credit = self.credit.checked_sub(credit_amount)?;

        let collateral = safe_add!(self.collateral, credit_amount);

        Some(Self {
            collateral,
            credit,
            ..self
        })
    }

    fn deposit(self, deposit: Collateral) -> Self {
        let collateral = safe_add!(self.collateral, deposit);
        Self { collateral, ..self }
    }
}

impl DebtIncreaseRequired {
    fn try_with_fee(self, max_ltv: MaxLtv, fee: Option<AdvanceFee>) -> Option<IssueDebt> {
        let max_debt = max_ltv
            .rate()
            .apply_u128(self.cdp.collateral)
            .expect("max ltv <= 100%");

        let debt = self
            .cdp
            .debt
            .checked_add(self.amount)
            .expect("debt balance should never overflow");

        if debt > max_debt {
            return None;
        }

        let Some(fee) = fee else {
            return Some(IssueDebt {
                cdp: Cdp { debt, ..self.cdp },
                amount: self.amount,
                fee: None,
            });
        };

        let fee_amount = fee
            .rate()
            .apply_u128(self.amount)
            .expect("advance fee <= 100%");

        let max_debt_buffer = max_debt.abs_diff(debt);

        if max_debt_buffer >= fee_amount {
            let debt = debt.checked_add(fee_amount).expect("debt <= max debt");

            return Some(IssueDebt {
                cdp: Cdp { debt, ..self.cdp },
                amount: self.amount,
                fee: Some(fee_amount),
            });
        }

        let amount = self
            .amount
            .checked_sub(max_debt_buffer.abs_diff(fee_amount))
            .expect("debt increase >= fee");

        Some(IssueDebt {
            cdp: Cdp {
                debt: max_debt,
                ..self.cdp
            },
            amount,
            fee: Some(fee_amount),
        })
    }
}

pub struct ConfigureHubImpl<'a> {
    vaults: &'a dyn Vaults,
    mint: &'a dyn SyntheticMint,
}

pub struct HubImpl<'a> {
    vaults: &'a dyn Vaults,
    balance_sheet: &'a dyn BalanceSheet,
    advance_fee_oracle: &'a dyn AdvanceFeeOracle,
}

pub fn configure<'a>(vaults: &'a dyn Vaults, mint: &'a dyn SyntheticMint) -> ConfigureHubImpl<'a> {
    ConfigureHubImpl { vaults, mint }
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

struct Response {
    cmds: Vec<Cmd>,
    vault: VaultId,
    account: Account,
    cdp: Cdp,
    updated_cdp: Option<Cdp>,
    balance_sheet: Option<BalanceSheetDiff>,
}

impl Response {
    fn new(vault: &VaultId, account: &Account, cdp: Cdp) -> Self {
        Self {
            cmds: vec![],
            vault: vault.clone(),
            account: account.clone(),
            cdp,
            updated_cdp: None,
            balance_sheet: None,
        }
    }

    fn cdp(&self) -> Cdp {
        self.updated_cdp.unwrap_or(self.cdp)
    }

    fn balance_sheet(&self) -> Option<BalanceSheetState> {
        self.balance_sheet
            .as_ref()
            .map(BalanceSheetDiff::balance_sheet)
    }

    fn redemption_rate(&self) -> Option<RedemptionRate> {
        self.balance_sheet
            .as_ref()
            .map(BalanceSheetDiff::redemption_rate)
    }

    fn push_cmd(&mut self, cmd: impl Into<Cmd>) -> &mut Self {
        self.cmds.push(cmd.into());
        self
    }

    fn set_cdp(&mut self, updated_cdp: Cdp) -> &mut Self {
        self.updated_cdp = Some(updated_cdp);
        self
    }

    fn with_balance_sheet_diff(&mut self, balance_sheet: BalanceSheetDiff) -> &mut Self {
        self.balance_sheet = Some(balance_sheet);
        self
    }

    fn set_balance_sheet(&mut self, balance_sheet: BalanceSheetState) -> &mut Self {
        self.balance_sheet.as_mut().unwrap().new = balance_sheet;
        self
    }

    fn add_mint_synthetic(
        &mut self,
        synthetic: &Synthetic,
        recipient: &Recipient,
        amount: SyntheticAmount,
    ) -> &mut Self {
        self.push_cmd(MintCmd::Mint {
            synthetic: synthetic.clone(),
            amount,
            recipient: recipient.clone(),
        })
    }

    fn add_burn_synthetic(&mut self, synthetic: &Synthetic, amount: SyntheticAmount) -> &mut Self {
        self.push_cmd(MintCmd::Burn {
            synthetic: synthetic.clone(),
            amount,
        })
    }

    fn add_vault_deposit(
        &mut self,
        vault: &VaultId,
        asset: &Asset,
        amount: DepositAmount,
        callback_recipient: &Recipient,
        callback_reason: VaultDepositReason,
    ) -> &mut Self {
        self.push_cmd(VaultCmd::Deposit {
            vault: vault.clone(),
            asset: asset.clone(),
            amount,
            callback_recipient: callback_recipient.clone(),
            callback_reason,
        })
    }

    fn add_vault_redeem(
        &mut self,
        vault: &VaultId,
        shares: &Asset,
        recipient: &Recipient,
        amount: SharesAmount,
    ) -> &mut Self {
        self.push_cmd(VaultCmd::Redeem {
            vault: vault.clone(),
            shares: shares.clone(),
            recipient: recipient.clone(),
            amount,
        })
    }

    fn done(self) -> (Cdp, Vec<Cmd>) {
        let mut cmds = self.cmds;

        if let Some(balance_sheet_diff) = self.balance_sheet {
            cmds.extend(balance_sheet_diff.into_cmds(&self.vault));
        }

        let cdp = if let Some(updated_cdp) = self.updated_cdp {
            if updated_cdp.collateral != self.cdp.collateral {
                cmds.push_cmd(BalanceSheetCmd::SetAccountCollateral {
                    vault: self.vault.clone(),
                    account: self.account.clone(),
                    collateral: updated_cdp.collateral,
                });
            }

            if updated_cdp.debt != self.cdp.debt {
                cmds.push_cmd(BalanceSheetCmd::SetAccountDebt {
                    vault: self.vault.clone(),
                    account: self.account.clone(),
                    debt: updated_cdp.debt,
                });
            }

            if updated_cdp.credit != self.cdp.credit {
                cmds.push_cmd(BalanceSheetCmd::SetAccountCredit {
                    vault: self.vault.clone(),
                    account: self.account.clone(),
                    credit: updated_cdp.credit,
                });
            }

            if updated_cdp.spr != self.cdp.spr {
                cmds.push_cmd(BalanceSheetCmd::SetAccountSumPaymentRatio {
                    vault: self.vault.clone(),
                    account: self.account.clone(),
                    spr: updated_cdp.spr,
                });
            }

            updated_cdp
        } else {
            self.cdp
        };

        (cdp, cmds)
    }
}

impl<'a> HubImpl<'a> {
    fn collateral_shares_pool(&self, vault: &VaultId) -> SharesPool {
        SharesPool {
            shares: self
                .balance_sheet
                .collateral_shares(vault)
                .unwrap_or_default(),
            quota: self
                .balance_sheet
                .collateral_balance(vault)
                .unwrap_or_default(),
        }
    }

    fn reserve_shares_pool(&self, vault: &VaultId) -> SharesPool {
        SharesPool {
            shares: self.balance_sheet.reserve_shares(vault).unwrap_or_default(),
            quota: self
                .balance_sheet
                .reserve_balance(vault)
                .unwrap_or_default(),
        }
    }

    fn redemption_rate(&self, vault: &VaultId) -> Option<RedemptionRate> {
        let total_shares_issued = self.vaults.total_shares_issued(vault);
        let total_deposit_value = self.vaults.total_deposits_value(vault);

        RedemptionRate::new(total_shares_issued, total_deposit_value)
    }

    fn collateral_yield_fee(&self, vault: &VaultId) -> CollateralYieldFee {
        self.vaults.collateral_yield_fee(vault).unwrap_or_default()
    }

    fn reserve_yield_fee(&self, vault: &VaultId) -> ReserveYieldFee {
        self.vaults.reserve_yield_fee(vault).unwrap_or_default()
    }

    fn amo_allocation(&self, vault: &VaultId) -> AmoAllocation {
        self.vaults.amo_allocation(vault).unwrap_or_default()
    }

    fn treasury_shares(&self, vault: &VaultId) -> TreasuryShares {
        self.balance_sheet
            .treasury_shares(vault)
            .unwrap_or_default()
    }

    fn amo_shares(&self, vault: &VaultId) -> AmoShares {
        self.balance_sheet.amo_shares(vault).unwrap_or_default()
    }

    fn overall_sum_payment_ratio(&self, vault: &VaultId) -> SumPaymentRatio {
        self.balance_sheet
            .overall_sum_payment_ratio(vault)
            .unwrap_or_else(SumPaymentRatio::zero)
    }

    fn balance_sheet_state(
        &self,
        vault: &VaultId,
        collateral_pool: CollateralPool,
        reserve_pool: ReservePool,
    ) -> BalanceSheetState {
        BalanceSheetState {
            collateral_pool,
            reserve_pool,
            treasury_shares: self.treasury_shares(vault),
            amo_shares: self.amo_shares(vault),
            spr: self.overall_sum_payment_ratio(vault),
        }
    }

    fn max_ltv(&self, vault: &VaultId) -> MaxLtv {
        self.vaults.max_ltv(vault).unwrap_or_default()
    }

    fn advance_fee(&self, vault: &VaultId, recipient: &Recipient) -> AdvanceFee {
        let Some(oracle) = self.vaults.advance_fee_oracle(vault) else {
            return self.vaults.fixed_advance_fee(vault).unwrap_or_default();
        };

        self.advance_fee_oracle
            .advance_fee(&oracle, recipient)
            .unwrap_or_default()
    }

    fn cdp(&self, vault: &VaultId, account: &Account) -> Cdp {
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
            .unwrap_or_else(SumPaymentRatio::zero);

        Cdp {
            collateral,
            debt,
            credit,
            spr,
        }
    }

    fn update_balance_sheet(&self, vault: &VaultId) -> Result<Option<BalanceSheetDiff>, Error> {
        let collateral_pool = self.collateral_shares_pool(vault);
        let reserve_pool = self.reserve_shares_pool(vault);

        let Some(redemption_rate) = self.redemption_rate(vault) else {
            return Ok(None);
        };

        let collateral_pool_surplus = collateral_pool.surplus(redemption_rate)?;

        let collateral_pool_payments = collateral_pool_surplus.map(|surplus| {
            surplus.payments(
                self.collateral_yield_fee(vault).rate(),
                self.amo_allocation(vault).rate(),
            )
        });

        let reserve_pool_surplus = reserve_pool.surplus(redemption_rate)?;

        let reserve_pool_payments = reserve_pool_surplus.map(|surplus| {
            surplus.payments(
                self.reserve_yield_fee(vault).rate(),
                self.amo_allocation(vault).rate(),
            )
        });

        let payments = match (collateral_pool_payments, reserve_pool_payments) {
            (Some(payments), None) => payments,
            (None, Some(payments)) => payments,
            (Some(cpp), Some(rpp)) => cpp.combine_with(rpp),

            // nothing to do if no payments
            (None, None) => return Ok(None),
        };

        let old = self.balance_sheet_state(vault, collateral_pool, reserve_pool);

        let new = old.update(collateral_pool_surplus, reserve_pool_surplus, payments);

        Ok(Some(BalanceSheetDiff {
            old,
            new,
            redemption_rate,
        }))
    }

    fn _evaluate(&self, vault: &VaultId, account: &Account) -> Result<Response, Error> {
        let cdp = self.cdp(vault, account);

        let mut response = Response::new(vault, account, cdp);

        let Some(balance_sheet_diff) = self.update_balance_sheet(vault)? else {
            return Ok(response);
        };

        let updated_cdp = response.cdp.update(balance_sheet_diff.balance_sheet().spr);

        response
            .set_cdp(updated_cdp)
            .with_balance_sheet_diff(balance_sheet_diff);

        Ok(response)
    }
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
            .underlying_asset_decimals(&synthetic)
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

impl<'a> Hub for HubImpl<'a> {
    fn evaluate(&self, vault: VaultId, sender: Sender) -> Result<PositionResponse, Error> {
        let (cdp, cmds) = self._evaluate(&vault, &sender)?.done();

        Ok(PositionResponse { cmds, cdp })
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

        let mut response = self._evaluate(&vault, &recipient)?;

        response.add_vault_deposit(
            &vault,
            &deposit_asset,
            deposit_amount,
            &recipient,
            VaultDepositReason::Deposit,
        );

        let (_, cmds) = response.done();

        Ok(cmds)
    }

    fn advance(
        &self,
        vault: VaultId,
        sender: Sender,
        advance_amount: Debt,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if advance_amount == 0 {
            return Err(Error::CannotAdvanceZero);
        }

        if self
            .vaults
            .deposit_proxy(&vault)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        let mut response = self._evaluate(&vault, &recipient)?;

        let advance_kind = response
            .cdp()
            .advance(advance_amount)
            .ok_or(Error::NotEnoughCollateral)?;

        let synthetic = self.vaults.synthetic_asset(&vault);

        match advance_kind {
            AdvanceKind::CoveredByCredit(cdp) => {
                response
                    .set_cdp(cdp)
                    .add_mint_synthetic(&synthetic, &recipient, advance_amount);
            }

            AdvanceKind::DebtIncreaseRequired(debt_increase) => {
                let fee_recipient = self.vaults.advance_fee_recipient(&vault);

                let max_ltv = self.max_ltv(&vault);

                let advance_fee = fee_recipient
                    .is_some()
                    .then(|| self.advance_fee(&vault, &recipient));

                let IssueDebt { cdp, amount, fee } = debt_increase
                    .try_with_fee(max_ltv, advance_fee)
                    .ok_or(Error::MaxLtvExceeded)?;

                response
                    .set_cdp(cdp)
                    .add_mint_synthetic(&synthetic, &recipient, amount);

                if let Some((fee_recipient, fee_amount)) = fee_recipient.zip(fee) {
                    response.add_mint_synthetic(&synthetic, &fee_recipient, fee_amount);
                }
            }
        }

        let (_, cmds) = response.done();

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

        let mut response = self._evaluate(&vault, &sender)?;

        if response.cdp().debt == 0 {
            return Err(Error::CannotRepayZero);
        }

        response.add_vault_deposit(
            &vault,
            &deposit_asset,
            deposit_amount,
            &sender,
            VaultDepositReason::RepayUnderlying,
        );

        let (_, cmds) = response.done();

        Ok(cmds)
    }

    fn repay_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
    ) -> Result<PositionResponse, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if synthetic_amount == 0 {
            return Err(Error::CannotRepayZero);
        }

        if synthetic_asset != self.vaults.synthetic_asset(&vault) {
            return Err(Error::InvalidSyntheticAsset);
        }

        let mut response = self._evaluate(&vault, &sender)?;

        if response.cdp().debt == 0 {
            return Err(Error::CannotRepayZero);
        }

        let cdp = response.cdp().repay(synthetic_amount);

        response
            .set_cdp(cdp)
            .add_burn_synthetic(&synthetic_asset, synthetic_amount);

        let (cdp, cmds) = response.done();

        Ok(PositionResponse { cmds, cdp })
    }

    fn withdraw_collateral(
        &self,
        vault: VaultId,
        sender: Sender,
        collateral_amount: Collateral,
    ) -> Result<PositionResponse, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if collateral_amount == 0 {
            return Err(Error::CannotWithdrawZero);
        }

        let mut response = self._evaluate(&vault, &sender)?;

        let max_ltv = self.max_ltv(&vault);

        let cdp = response
            .cdp()
            .withdraw(max_ltv, collateral_amount)
            .ok_or(Error::NotEnoughCollateral)?;

        let (balance_sheet, redemption_rate) = response
            .balance_sheet()
            .zip(response.redemption_rate())
            .expect("checked: witdrawal available");

        let (balance_sheet, shares_amount) = balance_sheet
            .withdraw_collateral(redemption_rate, collateral_amount)
            .expect("checked: withdrawal available");

        let shares_asset = self.vaults.shares_asset(&vault);

        response
            .set_cdp(cdp)
            .set_balance_sheet(balance_sheet)
            .add_vault_redeem(&vault, &shares_asset, &sender, shares_amount);

        let (cdp, cmds) = response.done();

        Ok(PositionResponse { cmds, cdp })
    }

    fn self_liquidate_position(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        let mut response = self._evaluate(&vault, &sender)?;

        let (cdp, self_liquidation) = response
            .cdp()
            .self_liquidate()
            .ok_or(Error::NothingToLiquidate)?;

        response.set_cdp(cdp);

        if self_liquidation.withdrawal.is_some() || self_liquidation.repayment.is_some() {
            let (mut balance_sheet, redemption_rate) = response
                .balance_sheet()
                .zip(response.redemption_rate())
                .expect("checked: self liquidation repayment available");

            if let Some(repayment) = self_liquidation.repayment {
                balance_sheet = balance_sheet
                    .move_collateral_to_reserve(redemption_rate, repayment)
                    .expect("checked: self liquidation repayment available");
            }

            if let Some(withdrawal) = self_liquidation.withdrawal {
                let (updated_balance_sheet, shares_amount) = balance_sheet
                    .withdraw_collateral(redemption_rate, withdrawal)
                    .expect("checked: self liquidation withdrawal available");

                let shares = self.vaults.shares_asset(&vault);

                response.add_vault_redeem(&vault, &shares, &sender, shares_amount);

                balance_sheet = updated_balance_sheet;
            }

            response.set_balance_sheet(balance_sheet);
        }

        if let Some(credit) = self_liquidation.credit {
            let synthetic = self.vaults.synthetic_asset(&vault);

            response.add_mint_synthetic(&synthetic, &sender, credit);
        }

        let (_, cmds) = response.done();

        Ok(cmds)
    }

    fn convert_credit(
        &self,
        vault: VaultId,
        sender: Sender,
        credit_amount: Credit,
    ) -> Result<PositionResponse, Error> {
        if credit_amount == 0 {
            return Err(Error::CannotConvertZero);
        }

        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        let mut response = self._evaluate(&vault, &sender)?;

        if credit_amount > response.cdp().credit {
            return Err(Error::NotEnoughCredit);
        }

        let (balance_sheet, redemption_rate) = response
            .balance_sheet()
            .zip(response.redemption_rate())
            .expect("checked: credit available");

        let balance_sheet = balance_sheet
            .move_reserves_to_collateral(redemption_rate, credit_amount)
            .ok_or(Error::InsufficientReserveBalance)?;

        let cdp = response
            .cdp()
            .convert_credit(credit_amount)
            .expect("checked: credit available");

        response.set_cdp(cdp).set_balance_sheet(balance_sheet);

        let (cdp, cmds) = response.done();

        Ok(PositionResponse { cmds, cdp })
    }

    fn redeem_synthetic(
        &self,
        vault: VaultId,
        sender: Sender,
        synthetic_asset: Synthetic,
        synthetic_amount: SyntheticAmount,
        recipient: Recipient,
    ) -> Result<Vec<Cmd>, Error> {
        if synthetic_amount == 0 {
            return Err(Error::CannotMintZero);
        }

        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        if synthetic_asset != self.vaults.synthetic_asset(&vault) {
            return Err(Error::InvalidSyntheticAsset);
        }

        if self
            .vaults
            .redeem_proxy(&vault)
            .is_some_and(|proxy| sender != proxy)
        {
            return Err(UnauthorizedError.into());
        }

        let mut response = self._evaluate(&vault, &sender)?;

        let (balance_sheet, redemption_rate) = response
            .balance_sheet()
            .zip(response.redemption_rate())
            .ok_or(Error::InsufficientReserveBalance)?;

        let (balance_sheet, shares_amount) = balance_sheet
            .withdraw_reserve(redemption_rate, synthetic_amount)
            .ok_or(Error::InsufficientReserveBalance)?;

        let shares_asset = self.vaults.shares_asset(&vault);

        response
            .set_balance_sheet(balance_sheet)
            .add_vault_redeem(&vault, &shares_asset, &recipient, shares_amount)
            .add_burn_synthetic(&synthetic_asset, synthetic_amount);

        let (_, cmds) = response.done();

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

        let mut response = self._evaluate(&vault, &sender)?;

        response.add_vault_deposit(
            &vault,
            &deposit_asset,
            deposit_amount,
            &recipient,
            VaultDepositReason::Mint,
        );

        let (_, cmds) = response.done();

        Ok(cmds)
    }

    fn vault_deposit_callback(
        &self,
        vault: VaultId,
        recipient: Recipient,
        reason: VaultDepositReason,
        issued_shares: SharesAmount,
        deposit_value: DepositValue,
    ) -> Result<Vec<Cmd>, Error> {
        assert!(self.vaults.is_registered(&vault));

        let cmds = match reason {
            VaultDepositReason::Deposit => {
                let SharesPool { shares, quota } = self
                    .collateral_shares_pool(&vault)
                    .add(issued_shares, deposit_value);

                let Cdp { collateral, .. } = self.cdp(&vault, &recipient).deposit(deposit_value);

                cmds![
                    BalanceSheetCmd::SetCollateralShares {
                        vault: vault.clone(),
                        shares
                    },
                    BalanceSheetCmd::SetCollateralBalance {
                        vault: vault.clone(),
                        balance: quota
                    },
                    BalanceSheetCmd::SetAccountCollateral {
                        vault: vault.clone(),
                        account: recipient.clone(),
                        collateral
                    }
                ]
            }

            VaultDepositReason::RepayUnderlying => {
                let SharesPool { shares, quota } = self
                    .reserve_shares_pool(&vault)
                    .add(issued_shares, deposit_value);

                let Cdp { debt, credit, .. } = self.cdp(&vault, &recipient).repay(deposit_value);

                let mut cmds = cmds![
                    BalanceSheetCmd::SetReserveShares {
                        vault: vault.clone(),
                        shares
                    },
                    BalanceSheetCmd::SetReserveBalance {
                        vault: vault.clone(),
                        balance: quota
                    },
                    BalanceSheetCmd::SetAccountDebt {
                        vault: vault.clone(),
                        account: recipient.clone(),
                        debt
                    }
                ];

                if credit > 0 {
                    cmds.push_cmd(BalanceSheetCmd::SetAccountCredit {
                        vault: vault.clone(),
                        account: recipient.clone(),
                        credit,
                    });
                }

                cmds
            }

            VaultDepositReason::Mint => {
                let SharesPool { shares, quota } = self
                    .reserve_shares_pool(&vault)
                    .add(issued_shares, deposit_value);

                let synthetic = self.vaults.synthetic_asset(&vault);

                cmds![
                    BalanceSheetCmd::SetReserveShares {
                        vault: vault.clone(),
                        shares
                    },
                    BalanceSheetCmd::SetReserveBalance {
                        vault,
                        balance: quota
                    },
                    MintCmd::Mint {
                        synthetic,
                        amount: deposit_value,
                        recipient
                    }
                ]
            }
        };

        Ok(cmds)
    }

    fn claim_treasury_shares(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        let treasury = self.balance_sheet.treasury().ok_or(Error::NoTreasurySet)?;

        if sender != treasury {
            return Err(UnauthorizedError.into());
        }

        let response = self._evaluate(&vault, &sender)?;

        let balance_sheet = response.balance_sheet().ok_or(Error::NothingToClaim)?;

        if balance_sheet.treasury_shares == 0 {
            return Err(Error::NothingToClaim);
        }

        let shares = self.vaults.shares_asset(&vault);

        Ok(cmds![
            BalanceSheetCmd::SetTreasuryShares { vault, shares: 0 },
            BalanceSheetCmd::SendShares {
                shares,
                amount: balance_sheet.treasury_shares,
                recipient: treasury
            }
        ])
    }

    fn claim_amo_shares(&self, vault: VaultId, sender: Sender) -> Result<Vec<Cmd>, Error> {
        if !self.vaults.is_registered(&vault) {
            return Err(Error::VaultNotRegistered);
        }

        let amo = self.vaults.amo(&vault).ok_or(Error::NoAmoSet)?;

        let response = self._evaluate(&vault, &sender)?;

        let balance_sheet = response.balance_sheet().ok_or(Error::NothingToClaim)?;

        if balance_sheet.amo_shares == 0 {
            return Err(Error::NothingToClaim);
        }

        let shares = self.vaults.shares_asset(&vault);

        Ok(cmds![
            BalanceSheetCmd::SetAmoShares { vault, shares: 0 },
            BalanceSheetCmd::SendShares {
                shares,
                amount: balance_sheet.treasury_shares,
                recipient: amo
            }
        ])
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
