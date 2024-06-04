use num::{FixedU256, U256};

use crate::{
    vault::{RedemptionRate, SharesAmount},
    Rate,
};

use super::{AdvanceFee, AmoAllocation, CollateralYieldFee, MaxLtv, ReserveYieldFee};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("vault shares have suffered a loss in value")]
pub struct LossError;

pub type CollateralPoolShares = SharesAmount;
pub type ReservePoolShares = SharesAmount;
pub type TreasuryShares = SharesAmount;
pub type AmoShares = SharesAmount;
pub type Collateral = u128;
pub type Debt = u128;
pub type Credit = u128;
pub type FeeAmount = u128;

#[derive(Debug, Clone, Copy)]
pub struct SharesPool {
    pub shares: SharesAmount,
    pub quota: Collateral,
}

// helper macro to ensure that unexpected overflows always panic regardless of compile options
macro_rules! safe_add {
    ($lhs:expr, $rhs:expr) => {
        $lhs.checked_add($rhs)
            .unwrap_or_else(|| panic!("overflow adding {} to {}", $lhs, $rhs))
    };
}

/// Σ x/y - where x is a debt payment and y is the collateral balance at the time of payment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct SumPaymentRatio(FixedU256);

impl SumPaymentRatio {
    pub const fn raw(x: U256) -> Self {
        Self(FixedU256::raw(x))
    }

    pub const fn zero() -> SumPaymentRatio {
        Self(FixedU256::raw(U256::zero()))
    }

    pub const fn fixed_u256(self) -> FixedU256 {
        self.0
    }

    pub const fn into_raw(self) -> U256 {
        self.0.into_raw()
    }
}

// NOTE: `Vault` & `Cdp` intentionally do not implement `Copy`.
// Updates should consume the old value to avoid mistakenly using a stale binding.

#[derive(Debug, Clone)]
pub struct Vault {
    pub collateral_pool: SharesPool,
    pub reserve_pool: SharesPool,
    pub treasury_shares: TreasuryShares,
    pub amo_shares: AmoShares,
    pub spr: SumPaymentRatio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Cdp {
    pub collateral: Collateral,
    pub debt: Debt,
    pub credit: Credit,
    pub spr: SumPaymentRatio,
}

#[derive(Debug, Clone, Copy)]
struct Surplus {
    shares: SharesAmount,
}

impl std::ops::Sub<Surplus> for SharesPool {
    type Output = SharesPool;

    fn sub(self, rhs: Surplus) -> Self::Output {
        SharesPool {
            shares: self
                .shares
                .checked_sub(rhs.shares)
                .expect("always: surplus shares < pool shares"),
            ..self
        }
    }
}

fn share_pool_surplus(
    pool: SharesPool,
    redemption_rate: RedemptionRate,
) -> Result<Option<Surplus>, LossError> {
    if pool.shares == 0 {
        return Ok(None);
    }

    let pool_shares_value = redemption_rate.shares_to_deposits(pool.shares);

    if pool_shares_value < pool.quota {
        return Err(LossError);
    }

    if pool_shares_value == pool.quota {
        return Ok(None);
    }

    let value = pool_shares_value
        .checked_sub(pool.quota)
        .expect("checked: pool shares value > pool quota");

    let shares = redemption_rate.deposits_to_shares(value);

    Ok(Some(Surplus { shares }))
}

struct Payments {
    treasury_shares: SharesAmount,
    amo_shares: SharesAmount,
    reserve_shares: SharesAmount,
}

impl std::ops::Add for Payments {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            treasury_shares: safe_add!(self.treasury_shares, rhs.treasury_shares),
            amo_shares: safe_add!(self.amo_shares, rhs.amo_shares),
            reserve_shares: safe_add!(self.reserve_shares, rhs.reserve_shares),
        }
    }
}

fn payments(surplus: Surplus, treasury_fee: Rate, amo_allocation: Rate) -> Payments {
    let treasury_shares = treasury_fee
        .apply_u128(surplus.shares)
        .expect("always: treasury fee <= 100%");

    let leftover_shares = surplus
        .shares
        .checked_sub(treasury_shares)
        .expect("always: treasury shares <= surplus shares");

    let amo_shares = amo_allocation
        .apply_u128(leftover_shares)
        .expect("always: amo allocation <= 100%");

    let reserve_shares = leftover_shares
        .checked_sub(amo_shares)
        .expect("always: amo shares <= leftover shares");

    Payments {
        treasury_shares,
        amo_shares,
        reserve_shares,
    }
}

fn apply_payments(
    vault: Vault,
    payments: Payments,
    redemption_rate: RedemptionRate,
) -> (Vault, Debt) {
    let treasury_shares = safe_add!(vault.treasury_shares, payments.treasury_shares);

    let amo_shares = safe_add!(vault.amo_shares, payments.amo_shares);

    let reserve_pool_shares = safe_add!(vault.reserve_pool.shares, payments.reserve_shares);

    let reserve_share_payment_value = redemption_rate.shares_to_deposits(payments.reserve_shares);

    let reserve_pool_quota = safe_add!(vault.reserve_pool.quota, reserve_share_payment_value);

    let amo_shares_payment_value = redemption_rate.shares_to_deposits(payments.amo_shares);

    let total_debt_payment = safe_add!(reserve_share_payment_value, amo_shares_payment_value);

    (
        Vault {
            reserve_pool: SharesPool {
                shares: reserve_pool_shares,
                quota: reserve_pool_quota,
            },
            treasury_shares,
            amo_shares,
            ..vault
        },
        total_debt_payment,
    )
}

enum Status {
    CollateralYieldOnly(Surplus),
    ReserveYieldOnly(Surplus),
    Both {
        collateral: Surplus,
        reserve: Surplus,
    },
}

fn subtract_collateral_pool_surplus(mut vault: Vault, surplus: Surplus) -> Vault {
    vault.collateral_pool = vault.collateral_pool - surplus;
    vault
}

fn subtract_reserve_pool_surplus(mut vault: Vault, surplus: Surplus) -> Vault {
    vault.reserve_pool = vault.reserve_pool - surplus;
    vault
}

fn increase_sum_payment_ratio(
    spr: SumPaymentRatio,
    payment: Debt,
    collateral: Collateral,
) -> SumPaymentRatio {
    let increase = FixedU256::from_u128(payment)
        .checked_div(FixedU256::from_u128(collateral))
        .expect("checked: collateral > 0");

    spr.fixed_u256()
        .checked_add(increase)
        .map(SumPaymentRatio)
        .expect("never: sum payment ratio overflow")
}

fn increase_vault_spr(mut vault: Vault, payment: Debt) -> Vault {
    if payment == 0 || vault.collateral_pool.quota == 0 {
        return vault;
    }

    vault.spr = increase_sum_payment_ratio(vault.spr, payment, vault.collateral_pool.quota);

    vault
}

pub trait Lazy<T> {
    fn get(&self) -> T;
}

impl<T, F> Lazy<T> for F
where
    F: Fn() -> T,
{
    fn get(&self) -> T {
        (self)()
    }
}

/// If vault shares have increased in value, perform re-balancing accordingly and return `Ok(Some(<updated vault position>))`.
/// In the case of where there is no increase in value, Ok(None) is returned.
/// `Err(LossError)` is returned if a loss in share value in detected.
pub fn update_vault(
    vault: Vault,
    redemption_rate: Option<RedemptionRate>,
    amo_allocation: impl Lazy<AmoAllocation>,
    collateral_treasury_fee: impl Lazy<CollateralYieldFee>,
    reserve_treasury_fee: impl Lazy<ReserveYieldFee>,
) -> Result<Option<Vault>, LossError> {
    // Step 1: get the current vault shares redemption rate (if one exists otherwise no update can occur)
    let Some(redemption_rate) = redemption_rate else {
        return Ok(None);
    };

    // Step 2: calculate any 'surplus' on either pool of shares (collateral or reserves)
    let collateral_pool_surplus = share_pool_surplus(vault.collateral_pool, redemption_rate)?;
    let reserve_pool_surplus = share_pool_surplus(vault.reserve_pool, redemption_rate)?;

    // Step 3: determine current position status based on surplus findings
    let status = match (collateral_pool_surplus, reserve_pool_surplus) {
        // no surplus found means no increase in shares value
        (None, None) => return Ok(None),
        (None, Some(reserve_surplus)) => Status::ReserveYieldOnly(reserve_surplus),
        (Some(collateral_surplus), None) => Status::CollateralYieldOnly(collateral_surplus),
        (Some(collateral), Some(reserve)) => Status::Both {
            collateral,
            reserve,
        },
    };

    let amo_allocation = amo_allocation.get();

    // Step 4: based on the current position status,
    // calculate the total payments that need to be made to the various balances:
    // - Treasury shares (unclaimed)
    // - AMO shares (unclaimed)
    // - Reserve Pool
    let payments = match status {
        Status::CollateralYieldOnly(surplus) => {
            let treasury_fee = collateral_treasury_fee.get();

            payments(surplus, treasury_fee.rate(), amo_allocation.rate())
        }

        // In this case the treasury fee is always 100% (i.e. 1.0) because it implies there are no
        // collateral pool entrants.
        Status::ReserveYieldOnly(surplus) => payments(surplus, Rate::one(), amo_allocation.rate()),

        // combine the payments arising from each set of surplus & fees together
        Status::Both {
            collateral,
            reserve,
        } => {
            let cp_treasury_fee = collateral_treasury_fee.get();
            let rp_treasury_fee = reserve_treasury_fee.get();

            payments(collateral, cp_treasury_fee.rate(), amo_allocation.rate())
                + payments(reserve, rp_treasury_fee.rate(), amo_allocation.rate())
        }
    };

    // Step 5: based on the current position status, subtract the surplus shares from the pools
    let vault = match status {
        Status::CollateralYieldOnly(surplus) => subtract_collateral_pool_surplus(vault, surplus),
        Status::ReserveYieldOnly(surplus) => subtract_reserve_pool_surplus(vault, surplus),
        Status::Both {
            collateral,
            reserve,
        } => {
            let vault = subtract_collateral_pool_surplus(vault, collateral);
            subtract_reserve_pool_surplus(vault, reserve)
        }
    };

    // Step 6: apply payments to corresponding balances, returning the vault and the total debt payment (before AMO split)
    let (vault, total_debt_payment) = apply_payments(vault, payments, redemption_rate);

    // Step 7: increase the sum payment ratio by total debt paymebt / collateral pool quota
    let vault = increase_vault_spr(vault, total_debt_payment);

    Ok(Some(vault))
}

pub fn repay(cdp: Cdp, amount: Debt) -> Cdp {
    if cdp.debt == 0 {
        let credit = safe_add!(cdp.credit, amount);

        return Cdp { credit, ..cdp };
    }

    if amount <= cdp.debt {
        let debt = cdp.debt.saturating_sub(amount);

        return Cdp { debt, ..cdp };
    }

    let credit_increase = cdp.debt.abs_diff(amount);

    let credit = safe_add!(cdp.credit, credit_increase);

    Cdp {
        credit,
        debt: 0,
        ..cdp
    }
}

pub fn update_cdp(vault: &Vault, cdp: Cdp) -> Cdp {
    let vault_spr = vault.spr.fixed_u256();

    let cdp_spr = cdp.spr.fixed_u256();

    // check if CDP is already up-to-date
    if vault_spr == cdp_spr {
        return cdp;
    }

    let diff = vault_spr
        .checked_sub(cdp_spr)
        .expect("always: vault spr >= account spr");

    let debt_payment = diff
        .checked_mul(FixedU256::from_u128(cdp.collateral))
        .expect("never: account debt payment overflow")
        .floor();

    let cdp = repay(cdp, debt_payment);

    Cdp {
        spr: vault.spr,
        ..cdp
    }
}

pub struct Advance {
    /// The updated CDP
    pub cdp: Cdp,
    /// The amount to mint to the CDP owner
    pub amount: Debt,
    /// The amount to mint to the fee recipient
    pub fee: Option<FeeAmount>,
}

pub fn advance(
    cdp: Cdp,
    amount: Debt,
    max_ltv: impl Lazy<MaxLtv>,
    advance_fee: impl Lazy<Option<AdvanceFee>>,
) -> Option<Advance> {
    // check if amount falls wholely within available credit
    if amount <= cdp.credit {
        let credit = cdp
            .credit
            .checked_sub(amount)
            .expect("checked: credit >= amount");

        return Some(Advance {
            cdp: Cdp { credit, ..cdp },
            amount,
            fee: None,
        });
    }

    // the difference between credit balance and requested amount
    let debt_increase = cdp.credit.abs_diff(amount);

    let debt = cdp.debt.checked_add(debt_increase)?;

    // reject anything over 100% LTV
    if cdp.collateral < debt {
        return None;
    }

    // calculate max debt balance allowed based on collateral and Max LTV
    let max_debt = max_ltv
        .get()
        .rate()
        .apply_u128(cdp.collateral)
        .expect("always: max ltv <= 100%");

    // reject anything over Max LTV
    if debt > max_debt {
        return None;
    }

    // check if there is to be an advance fee applied
    let Some(advance_fee) = advance_fee.get() else {
        // nothing left to do if not
        return Some(Advance {
            cdp: Cdp {
                credit: 0,
                debt,
                ..cdp
            },
            amount,
            fee: None,
        });
    };

    // calculate fee amount based on the *debt increase* amount (credit use is not charged)
    let fee = advance_fee
        .rate()
        .apply_u128(debt_increase)
        .expect("always: fee <= 100%");

    // calculate available buffer between the new debt level and the maximum debt level
    let buffer = debt.abs_diff(max_debt);

    // check if the fee can be added on as additional debt
    if fee <= buffer {
        let debt = safe_add!(debt, fee);

        return Some(Advance {
            cdp: Cdp {
                credit: 0,
                debt,
                ..cdp
            },
            amount,
            fee: Some(fee),
        });
    }

    // max out the debt: takes the part fee that exceeds the buffer from the amount to be minted to the CDP owner
    let fee_remainder = buffer.abs_diff(fee);

    let amount = amount
        .checked_sub(fee_remainder)
        .expect("always: amount > fee remainder");

    Some(Advance {
        cdp: Cdp {
            credit: 0,
            debt: max_debt,
            ..cdp
        },
        amount,
        fee: Some(fee),
    })
}

fn withdraw_cdp_collateral(cdp: Cdp, max_ltv: MaxLtv, amount: Collateral) -> Option<Cdp> {
    // reject if amount greater than deposited collateral
    if amount > cdp.collateral {
        return None;
    }

    let collateral = cdp
        .collateral
        .checked_sub(amount)
        .expect("checked: amount <= collateral balance");

    // if there is no debt then always allow
    if cdp.debt == 0 {
        return Some(Cdp { collateral, ..cdp });
    }

    // reject if 100% LTV exceeded
    if collateral < cdp.debt {
        return None;
    }

    let proposed_ltv = Rate::from_ratio(cdp.debt, collateral).expect("checked: collateral > 0");

    // reject if max LTV exceeded
    if proposed_ltv > max_ltv.rate() {
        return None;
    }

    Some(Cdp { collateral, ..cdp })
}

fn withdraw_vault_collateral(
    mut vault: Vault,
    redemption_rate: RedemptionRate,
    amount: Collateral,
) -> Option<(Vault, SharesAmount)> {
    let shares = redemption_rate.checked_deposits_to_shares(amount)?;

    vault.collateral_pool.shares = vault.collateral_pool.shares.checked_sub(shares)?;

    vault.collateral_pool.quota = vault.collateral_pool.quota.checked_sub(amount)?;

    Some((vault, shares))
}

pub fn add_vault_reserves(mut vault: Vault, amount: Collateral, shares: SharesAmount) -> Vault {
    vault.reserve_pool.shares = safe_add!(vault.reserve_pool.shares, shares);
    vault.reserve_pool.quota = safe_add!(vault.reserve_pool.quota, amount);
    vault
}

fn withdraw_vault_reserves(
    mut vault: Vault,
    redemption_rate: RedemptionRate,
    amount: Collateral,
) -> Option<(Vault, SharesAmount)> {
    let shares = redemption_rate.checked_deposits_to_shares(amount)?;

    vault.reserve_pool.shares = vault.reserve_pool.shares.checked_sub(shares)?;

    vault.reserve_pool.quota = vault.reserve_pool.quota.checked_sub(amount)?;

    Some((vault, shares))
}

pub fn add_vault_collateral(mut vault: Vault, amount: Collateral, shares: SharesAmount) -> Vault {
    vault.collateral_pool.shares = safe_add!(vault.collateral_pool.shares, shares);
    vault.collateral_pool.quota = safe_add!(vault.collateral_pool.quota, amount);
    vault
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WithdrawCollateralError {
    #[error(transparent)]
    VaultLoss(#[from] LossError),
    #[error("not enough collateral")]
    NotEnoughCollateral,
}

pub fn withdraw_collateral(
    vault: Vault,
    cdp: Cdp,
    amount: Collateral,
    max_ltv: MaxLtv,
    redemption_rate: Option<RedemptionRate>,
) -> Result<(Vault, Cdp, SharesAmount), WithdrawCollateralError> {
    let cdp = withdraw_cdp_collateral(cdp, max_ltv, amount)
        .ok_or(WithdrawCollateralError::NotEnoughCollateral)?;

    let redemption_rate = redemption_rate.ok_or(LossError)?;

    let (vault, shares) =
        withdraw_vault_collateral(vault, redemption_rate, amount).ok_or(LossError)?;

    Ok((vault, cdp, shares))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SelfLiquidateError {
    #[error(transparent)]
    VaultLoss(#[from] LossError),
    #[error("nothing to liquidate")]
    NothingToLiquidate,
    #[error("insufficient reserves")]
    InsufficientReserves,
}

pub struct SelfLiquidation {
    pub vault: Vault,
    pub cdp: Cdp,
    pub mint_credit: Option<Credit>,
    pub redeem_shares: Option<SharesAmount>,
}

pub fn self_liquidate(
    vault: Vault,
    cdp: Cdp,
    redemption_rate: Option<RedemptionRate>,
) -> Result<SelfLiquidation, SelfLiquidateError> {
    if cdp.collateral == 0 && cdp.credit == 0 {
        return Err(SelfLiquidateError::NothingToLiquidate);
    }

    let zeroed_cdp = Cdp {
        collateral: 0,
        debt: 0,
        credit: 0,
        spr: SumPaymentRatio::zero(),
    };

    let redemption_rate = redemption_rate.ok_or(LossError)?;

    // if there is credit, there is no debt to cancel out
    if cdp.credit > 0 {
        assert_eq!(cdp.debt, 0, "there can be no debt when there is credit");

        // if there is no collateral, just mint the credit
        if cdp.collateral == 0 {
            return Ok(SelfLiquidation {
                vault,
                cdp: zeroed_cdp,
                mint_credit: Some(cdp.credit),
                redeem_shares: None,
            });
        }

        let (vault, shares) =
            withdraw_vault_collateral(vault, redemption_rate, cdp.collateral).ok_or(LossError)?;

        return Ok(SelfLiquidation {
            vault,
            cdp: zeroed_cdp,
            mint_credit: Some(cdp.credit),
            redeem_shares: Some(shares),
        });
    }

    // if there is no debt, just withdraw all the collateral
    if cdp.debt == 0 {
        let (vault, shares) =
            withdraw_vault_collateral(vault, redemption_rate, cdp.collateral).ok_or(LossError)?;

        return Ok(SelfLiquidation {
            vault,
            cdp: zeroed_cdp,
            mint_credit: None,
            redeem_shares: Some(shares),
        });
    }

    // the amount to withdraw is the difference between collateral and debt
    let withdraw_amount = cdp.collateral.abs_diff(cdp.debt);

    // withdraw an equivalent amount of collateral as there is debt from the vault collateral pool
    let (vault, shares) =
        withdraw_vault_collateral(vault, redemption_rate, cdp.debt).ok_or(LossError)?;

    // add the shares/amount to the reserve pool
    let vault = add_vault_reserves(vault, cdp.debt, shares);

    // withdraw the remainder from the vault collateral pool
    let (vault, shares) =
        withdraw_vault_collateral(vault, redemption_rate, withdraw_amount).ok_or(LossError)?;

    Ok(SelfLiquidation {
        vault,
        cdp: zeroed_cdp,
        mint_credit: None,
        redeem_shares: Some(shares),
    })
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertCreditError {
    #[error(transparent)]
    VaultLoss(#[from] LossError),
    #[error("not enough credit")]
    NotEnoughCredit,
    #[error("insufficient reserves")]
    InsufficientReserves,
}

pub fn convert_credit(
    vault: Vault,
    mut cdp: Cdp,
    amount: Credit,
    redemption_rate: Option<RedemptionRate>,
) -> Result<(Vault, Cdp), ConvertCreditError> {
    if cdp.credit < amount {
        return Err(ConvertCreditError::NotEnoughCredit);
    }

    let redemption_rate = redemption_rate.ok_or(LossError)?;

    let (vault, shares) = withdraw_vault_reserves(vault, redemption_rate, amount)
        .ok_or(ConvertCreditError::InsufficientReserves)?;

    let shares_value = redemption_rate.shares_to_deposits(shares);

    let vault = add_vault_collateral(vault, shares_value, shares);

    cdp.credit = cdp
        .credit
        .checked_sub(amount)
        .expect("checked: credit >= amount");

    cdp.collateral = safe_add!(cdp.collateral, shares_value);

    Ok((vault, cdp))
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RedeemReservesError {
    #[error(transparent)]
    VaultLoss(#[from] LossError),
    #[error("insufficient reserves")]
    InsufficientReserves,
}

pub fn redeem_reserves(
    vault: Vault,
    amount: Collateral,
    redemption_rate: Option<RedemptionRate>,
) -> Result<(Vault, SharesAmount), RedeemReservesError> {
    if vault.reserve_pool.quota < amount {
        return Err(RedeemReservesError::InsufficientReserves);
    }

    let redemption_rate = redemption_rate.ok_or(LossError)?;

    let (vault, shares) =
        withdraw_vault_reserves(vault, redemption_rate, amount).ok_or(LossError)?;

    Ok((vault, shares))
}

pub fn deposit_collateral(
    vault: Vault,
    mut cdp: Cdp,
    amount: Collateral,
    shares: SharesAmount,
) -> (Vault, Cdp) {
    cdp.collateral = safe_add!(cdp.collateral, amount);

    let vault = add_vault_collateral(vault, amount, shares);

    (vault, cdp)
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("nothing to claim")]
pub struct NothingToClaimError;

pub fn claim_treasury_shares(vault: Vault) -> Result<(Vault, SharesAmount), NothingToClaimError> {
    let shares = vault.treasury_shares;

    if shares == 0 {
        return Err(NothingToClaimError);
    }

    let vault = Vault {
        treasury_shares: 0,
        ..vault
    };

    Ok((vault, shares))
}

pub fn claim_amo_shares(vault: Vault) -> Result<(Vault, SharesAmount), NothingToClaimError> {
    let shares = vault.amo_shares;

    if shares == 0 {
        return Err(NothingToClaimError);
    }

    let vault = Vault {
        amo_shares: 0,
        ..vault
    };

    Ok((vault, shares))
}

// #[cfg(test)]
// mod test {
//     use test_utils::prelude::*;

//     use super::{self_liquidate, *};

//     #[test]
//     fn stress_sum_payment_ratio() {
//         let mut collateral = 1_000_000u128;
//         let mut payment = collateral / 10;

//         // let mut spr = SumPaymentRatio::zero();

//         // // handle 1 million updates without overflow
//         // for _ in 0..1_000_000 {
//         //     spr = increase_sum_payment_ratio(spr, payment, collateral);
//         // }

//         let mut spr = SumPaymentRatio::zero();

//         // collateral balance would overflow before SPR
//         while collateral.abs_diff(u128::MAX) > payment {
//             spr = increase_sum_payment_ratio(spr, payment, collateral);
//             collateral = safe_add!(collateral, payment);
//             payment = collateral / 1000;
//         }
//     }

//     #[test]
//     fn update_positions() {
//         const INIT_DEPOSIT: u128 = 1_000_000;
//         const INIT_SHARES: u128 = 1_000_000_000_000_000_000;

//         let initial_vault = Vault {
//             collateral_pool: SharesPool {
//                 shares: INIT_SHARES,
//                 quota: INIT_DEPOSIT,
//             },
//             reserve_pool: SharesPool {
//                 shares: 0,
//                 quota: 0,
//             },
//             treasury_shares: 0,
//             amo_shares: 0,
//             spr: SumPaymentRatio::zero(),
//         };

//         let initial_cdp = Cdp {
//             collateral: INIT_DEPOSIT,
//             debt: INIT_DEPOSIT / 2,
//             credit: 0,
//             spr: SumPaymentRatio::zero(),
//         };

//         // 10% earned in yield since inital deposit
//         let total_deposit_value = (INIT_DEPOSIT * 11) / 10;
//         let redemption_rate = RedemptionRate::new(INIT_SHARES, total_deposit_value);

//         let updated_vault = update_vault(
//             initial_vault,
//             redemption_rate,
//             AmoAllocation::default,
//             CollateralYieldFee::default,
//             ReserveYieldFee::default,
//         )
//         .unwrap()
//         .unwrap();

//         let updated_cdp = update_cdp(&updated_vault, initial_cdp);

//         check(
//             &updated_vault,
//             expect![[r#"
//                 (
//                   collateral_pool: (
//                     shares: 909090909090909091,
//                     quota: 1000000,
//                   ),
//                   reserve_pool: (
//                     shares: 81818181818181819,
//                     quota: 90000,
//                   ),
//                   treasury_shares: 9090909090909090,
//                   amo_shares: 0,
//                   spr: (("0.08999999999999999999999999999999")),
//                 )"#]],
//         );

//         check(
//             &updated_cdp,
//             expect![[r#"
//                 (
//                   collateral: 1000000,
//                   debt: 410001,
//                   credit: 0,
//                   spr: (("0.08999999999999999999999999999999")),
//                 )"#]],
//         );

//         assert_eq!(
//             updated_vault.collateral_pool.shares
//                 + updated_vault.reserve_pool.shares
//                 + updated_vault.treasury_shares,
//             INIT_SHARES
//         );

//         let treasury_shares_value = redemption_rate
//             .unwrap()
//             .shares_to_deposits(updated_vault.treasury_shares);

//         check(treasury_shares_value, expect!["9999"]);

//         // within one
//         assert_wn!(
//             1,
//             updated_vault.collateral_pool.quota
//                 + updated_vault.reserve_pool.quota
//                 + treasury_shares_value,
//             total_deposit_value
//         );

//         // another 10% earned in yield since update
//         let total_deposit_value = (total_deposit_value * 11) / 10;
//         let redemption_rate = RedemptionRate::new(INIT_SHARES, total_deposit_value);

//         let updated_vault = update_vault(
//             updated_vault,
//             redemption_rate,
//             AmoAllocation::default,
//             CollateralYieldFee::default,
//             ReserveYieldFee::default,
//         )
//         .unwrap()
//         .unwrap();

//         let updated_cdp = update_cdp(&updated_vault, updated_cdp);

//         check(
//             &updated_vault,
//             expect![[r#"
//                 (
//                   collateral_pool: (
//                     shares: 826446280991735538,
//                     quota: 1000000,
//                   ),
//                   reserve_pool: (
//                     shares: 148760330578512398,
//                     quota: 179999,
//                   ),
//                   treasury_shares: 24793388429752064,
//                   amo_shares: 0,
//                   spr: (("0.17999899999999999999999999999999")),
//                 )"#]],
//         );

//         check(
//             updated_cdp,
//             expect![[r#"
//                 (
//                   collateral: 1000000,
//                   debt: 320003,
//                   credit: 0,
//                   spr: (("0.17999899999999999999999999999999")),
//                 )"#]],
//         );

//         assert_eq!(
//             updated_vault.collateral_pool.shares
//                 + updated_vault.reserve_pool.shares
//                 + updated_vault.treasury_shares,
//             INIT_SHARES
//         );

//         let treasury_shares_value = redemption_rate
//             .unwrap()
//             .shares_to_deposits(updated_vault.treasury_shares);

//         check(treasury_shares_value, expect!["29999"]);

//         assert_wn!(
//             2,
//             updated_vault.collateral_pool.quota
//                 + updated_vault.reserve_pool.quota
//                 + treasury_shares_value,
//             total_deposit_value
//         );
//     }

//     #[test]
//     fn self_liquidate_works() {
//         let cdp = Cdp {
//             collateral: 500_000,
//             debt: 102502,
//             credit: 0,
//             spr: SumPaymentRatio::raw(U256::from(61250485763402002484943965963110846293u128)),
//         };

//         let vault = Vault {
//             collateral_pool: SharesPool {
//                 shares: 413223140495867770,
//                 quota: 500000,
//             },
//             reserve_pool: SharesPool {
//                 shares: 233469421487603307,
//                 quota: 282498,
//             },
//             treasury_shares: 24794214876033055,
//             amo_shares: 0,
//             spr: SumPaymentRatio::raw(U256::from(61250485763402002484943965963110846293u128)),
//         };

//         let redemption_rate = RedemptionRate::new(671486776859504132, 812500);

//         let self_liquidation = self_liquidate(vault, cdp, redemption_rate).unwrap();

//         assert!(self_liquidation.mint_credit.is_none());

//         check(
//             self_liquidation.redeem_shares,
//             expect!["Some(328510339480737444)"],
//         );

//         check(
//             &self_liquidation.vault,
//             expect![[r#"
//                 (
//                   collateral_pool: (
//                     shares: 508582326766,
//                     quota: 0,
//                   ),
//                   reserve_pool: (
//                     shares: 318181713920406867,
//                     quota: 385000,
//                   ),
//                   treasury_shares: 24794214876033055,
//                   amo_shares: 0,
//                   spr: (("0.17999899999999999999999999999999")),
//                 )"#]],
//         );

//         check(
//             self_liquidation.cdp,
//             expect![[r#"
//                 (
//                   collateral: 0,
//                   debt: 0,
//                   credit: 0,
//                   spr: (("0.0")),
//                 )"#]],
//         );

//         let remaining_collateral_shares_value = redemption_rate
//             .unwrap()
//             .shares_to_deposits(self_liquidation.vault.collateral_pool.shares);

//         check(remaining_collateral_shares_value, expect!["0"]);
//     }
// }
