use std::collections::HashMap;

use num::FixedU256;
use test_utils::prelude::*;

use crate::vault::SHARES_DECIMAL_PLACES;

use super::*;

const ALREADY_REGISTERED_VAULT: &str = "already_registered_vault";
const SENDER: &str = "sender";
const SYNTHETIC: &str = "synthetic";
const EIGHT_DECIMAL_SYNTHETIC: &str = "eight_decimal_synthetic";
const VAULT: &str = "vault";
const VAULT_DEPOSIT_ASSET: &str = "vault_deposit_asset";
const VAULT_SHARES_ASSET: &str = "vault_shares_asset";

const fn shares_amount(n: u128) -> u128 {
    n * 10u128.pow(SHARES_DECIMAL_PLACES)
}

#[derive(Debug, Default, serde::Serialize)]
struct VaultMeta {
    deposits_enabled: bool,
    advance_enabled: bool,
    advance_fee_oracle: Option<Oracle>,
    advance_fee_recipient: Option<Recipient>,
    amo: Option<Amo>,
    deposit_proxy: Option<Proxy>,
    advance_proxy: Option<Proxy>,
    mint_proxy: Option<Proxy>,
    redeem_proxy: Option<Proxy>,
}

#[derive(Debug, Default, serde::Serialize)]
struct User {
    collateral: u128,
    debt: u128,
    credit: u128,
    spr: Option<SumPaymentRatio>,
}

#[derive(Debug, Default, serde::Serialize)]
struct Balances {
    users: HashMap<String, User>,
    collateral_shares: u128,
    collateral_balance: u128,
    reserve_shares: u128,
    reserve_balance: u128,
    treasury_shares: u128,
    amo_shares: u128,
    spr: Option<SumPaymentRatio>,
}

#[derive(Debug, serde::Serialize)]
struct Vault {
    synthetic: Synthetic,
    meta: VaultMeta,
    balances: Balances,
}

#[derive(Default)]
struct World {
    vaults: HashMap<String, Vault>,
    treasury: Option<Treasury>,
    oracle_advance_fee: Option<AdvanceFee>,
    total_deposits: TotalDepositsValue,
    total_issued_shares: TotalSharesIssued,
}

#[test]
fn deposit_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .deposit(
                "does_not_exist".into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn deposit_when_disabled_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["deposits disabled"],
    )
}

#[test]
fn deposit_not_from_configured_proxy_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                VaultCmd::SetDepositProxy {
                    vault: VAULT.into(),
                    proxy: "deposit_proxy".into()
                }
            ])
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["unauthorized"],
    )
}

#[test]
fn deposit_zero_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                }
            ])
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                0,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["cannot deposit zero"],
    )
}

#[test]
fn deposit_invalid_asset_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                }
            ])
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                "unknown_asset".into(),
                1000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["invalid deposit asset"],
    )
}

#[test]
fn deposit_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn deposit() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                }
            ])
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1000,
                SENDER.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: Deposit,
              )),
            ]"#]],
    )
}

#[test]
fn deposit_after_share_value_increase_sender_has_position() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1000,
                SENDER.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 909090909090909090910,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 81818181818181818182,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 90,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountCredit(
                vault: "vault",
                account: "sender",
                credit: 89,
              )),
              BalanceSheet(SetAccountSumPaymentRatio(
                vault: "vault",
                account: "sender",
                spr: (("0.08999999999999999999999999999999")),
              )),
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: Deposit,
              )),
            ]"#]],
    )
}

#[test]
fn deposit_after_share_value_increase_sender_with_no_position() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: "bob".into(),
                    collateral: 1_000
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .deposit(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1000,
                SENDER.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 909090909090909090910,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 81818181818181818182,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 90,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountSumPaymentRatio(
                vault: "vault",
                account: "sender",
                spr: (("0.08999999999999999999999999999999")),
              )),
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: Deposit,
              )),
            ]"#]],
    )
}

#[test]
fn advance_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .advance("does_not_exist".into(), SENDER.into(), 1_000, SENDER.into())
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn advance_when_disabled_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .advance(VAULT.into(), SENDER.into(), 1_000, SENDER.into())
            .unwrap_err(),
        expect!["advance disabled"],
    )
}

#[test]
fn advance_zero_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                }
            ])
            .hub()
            .advance(VAULT.into(), SENDER.into(), 0, SENDER.into())
            .unwrap_err(),
        expect!["cannot advance zero"],
    )
}

#[test]
fn advance_not_from_configured_proxy_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                VaultCmd::SetAdvanceProxy {
                    vault: VAULT.into(),
                    proxy: "advance_proxy".into()
                }
            ])
            .hub()
            .advance(VAULT.into(), SENDER.into(), 1_000, SENDER.into())
            .unwrap_err(),
        expect!["unauthorized"],
    )
}

#[test]
fn advance_without_enough_collateral_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 499
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .advance(VAULT.into(), SENDER.into(), 2, SENDER.into())
            .unwrap_err(),
        expect!["not enough collateral"],
    )
}

#[test]
fn advance_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .advance(VAULT.into(), SENDER.into(), 1_000, SENDER.into())
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn advance_debt() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 200
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .advance(VAULT.into(), SENDER.into(), 300, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 500,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 300,
                recipient: "sender",
              )),
            ]"#]],
    )
}

#[test]
fn advance_non_max_debt_with_fixed_advance_fee() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                VaultCmd::SetAdvanceFeeRecipient {
                    vault: VAULT.into(),
                    recipient: "treasury".into(),
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 200_000
                }
            ])
            .total_deposits(1_000_000)
            .total_shares_issued(shares_amount(1_000_000))
            .hub()
            .advance(VAULT.into(), SENDER.into(), 200_000, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 400499,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 200000,
                recipient: "sender",
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 499,
                recipient: "treasury",
              )),
            ]"#]],
    );
}

#[test]
fn advance_max_debt_with_fixed_advance_fee() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                VaultCmd::SetAdvanceFeeRecipient {
                    vault: VAULT.into(),
                    recipient: "treasury".into(),
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 200_000
                }
            ])
            .total_deposits(1_000_000)
            .total_shares_issued(shares_amount(1_000_000))
            .hub()
            .advance(VAULT.into(), SENDER.into(), 300_000, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 500000,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 299251,
                recipient: "sender",
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 749,
                recipient: "treasury",
              )),
            ]"#]],
    );
}

#[test]
fn advance_non_max_debt_with_advance_fee_oracle() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAdvanceEnabled {
                    vault: VAULT.into(),
                    enabled: true
                },
                VaultCmd::SetAdvanceFeeRecipient {
                    vault: VAULT.into(),
                    recipient: "treasury".into(),
                },
                VaultCmd::SetAdvanceFeeOracle {
                    vault: VAULT.into(),
                    oracle: "advance_fee_oracle".into(),
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 200_000
                }
            ])
            .total_deposits(1_000_000)
            .total_shares_issued(shares_amount(1_000_000))
            .advance_fee_oracle_rate(AdvanceFee::new(100).unwrap())
            .hub()
            .advance(VAULT.into(), SENDER.into(), 200_000, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 401999,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 200000,
                recipient: "sender",
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 1999,
                recipient: "treasury",
              )),
            ]"#]],
    );
}

#[test]
fn advance_all_credit() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            VaultCmd::SetAdvanceEnabled {
                vault: VAULT.into(),
                enabled: true
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        world
            .handle_cmds(response.cmds)
            .hub()
            .advance(VAULT.into(), SENDER.into(), 89, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountCredit(
                vault: "vault",
                account: "sender",
                credit: 0,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 89,
                recipient: "sender",
              )),
            ]"#]],
    );
}

#[test]
fn advance_from_credit_with_fee() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .advance_fee_oracle_rate(AdvanceFee::new(1_000).unwrap())
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            VaultCmd::SetAdvanceEnabled {
                vault: VAULT.into(),
                enabled: true
            },
            VaultCmd::SetAdvanceFeeRecipient {
                vault: VAULT.into(),
                recipient: "treasury".into(),
            },
            VaultCmd::SetAdvanceFeeOracle {
                vault: VAULT.into(),
                oracle: "advance_fee_oracle".into(),
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        world
            .handle_cmds(response.cmds)
            .hub()
            .advance(VAULT.into(), SENDER.into(), 489, SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 439,
              )),
              BalanceSheet(SetAccountCredit(
                vault: "vault",
                account: "sender",
                credit: 0,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 489,
                recipient: "sender",
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 39,
                recipient: "treasury",
              )),
            ]"#]],
    );
}

#[test]
fn repay_underlying_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .repay_underlying(
                "does_not_exist".into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
            )
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn repay_underlying_with_invalid_deposit_asset_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_underlying(VAULT.into(), SENDER.into(), "does_not_exist".into(), 1_000)
            .unwrap_err(),
        expect!["invalid deposit asset"],
    )
}

#[test]
fn repay_zero_underlying_assets_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_underlying(VAULT.into(), SENDER.into(), VAULT_DEPOSIT_ASSET.into(), 0)
            .unwrap_err(),
        expect!["cannot repay zero"],
    )
}

#[test]
fn repay_underlying_with_zero_debt_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_underlying(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
            )
            .unwrap_err(),
        expect!["nothing to repay"],
    )
}

#[test]
fn repay_underlying_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_underlying(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
            )
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn repay_underlying() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_underlying(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
            )
            .unwrap(),
        expect![[r#"
            [
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: RepayUnderlying,
              )),
            ]"#]],
    )
}

#[test]
fn repay_underlying_after_share_value_increase() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_underlying(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 909090909090909090910,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 81818181818181818182,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 90,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 411,
              )),
              BalanceSheet(SetAccountSumPaymentRatio(
                vault: "vault",
                account: "sender",
                spr: (("0.08999999999999999999999999999999")),
              )),
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: RepayUnderlying,
              )),
            ]"#]],
    )
}

#[test]
fn repay_synthetic_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .repay_synthetic(
                "does_not_exist".into(),
                SENDER.into(),
                SYNTHETIC.into(),
                1_000,
            )
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn repay_invalid_synthetic_asset_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), "unknown_asset".into(), 1_000)
            .unwrap_err(),
        expect!["invalid synthetic asset"],
    )
}

#[test]
fn repay_zero_synthetic_assets_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), SYNTHETIC.into(), 0)
            .unwrap_err(),
        expect!["cannot repay zero"],
    )
}

#[test]
fn repay_synthetic_with_zero_debt_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), SYNTHETIC.into(), 0)
            .unwrap_err(),
        expect!["cannot repay zero"],
    )
}

#[test]
fn repay_synthetic_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), SYNTHETIC.into(), 1_000)
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn repay_synthetic() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), SYNTHETIC.into(), 500)
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetAccountDebt(
                  vault: "vault",
                  account: "sender",
                  debt: 0,
                )),
                Mint(Burn(
                  synthetic: "synthetic",
                  amount: 500,
                )),
              ],
              cdp: (
                collateral: 1000,
                debt: 0,
                credit: 0,
                spr: (("0.0")),
              ),
            )"#]],
    )
}

#[test]
fn repay_synthetic_after_vault_shares_value_increase() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .repay_synthetic(VAULT.into(), SENDER.into(), SYNTHETIC.into(), 500)
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetAccountDebt(
                  vault: "vault",
                  account: "sender",
                  debt: 0,
                )),
                BalanceSheet(SetAccountCredit(
                  vault: "vault",
                  account: "sender",
                  credit: 89,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 909090909090909090910,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 81818181818181818182,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 90,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                Mint(Burn(
                  synthetic: "synthetic",
                  amount: 500,
                )),
              ],
              cdp: (
                collateral: 1000,
                debt: 0,
                credit: 89,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    )
}

#[test]
fn withdraw_collateral_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .withdraw_collateral("does_not_exist".into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn withdraw_zero_collateral_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 0)
            .unwrap_err(),
        expect!["cannot withdraw zero"],
    )
}

#[test]
fn withdraw_collateral_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn withdraw_collateral_without_deposits_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["not enough collateral"],
    )
}

#[test]
fn withdraw_collateral_exceeding_max_ltv_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 400
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 201)
            .unwrap_err(),
        expect!["not enough collateral"],
    )
}

#[test]
fn withdraw_collateral() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 400
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 200)
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 800000000000000000001,
                )),
                BalanceSheet(SetCollateralBalance(
                  vault: "vault",
                  balance: 800,
                )),
                BalanceSheet(SetAccountCollateral(
                  vault: "vault",
                  account: "sender",
                  collateral: 800,
                )),
                Vault(Redeem(
                  vault: "vault",
                  shares: "vault_shares_asset",
                  amount: 199999999999999999999,
                  recipient: "sender",
                )),
              ],
              cdp: (
                collateral: 800,
                debt: 400,
                credit: 0,
                spr: (("0.0")),
              ),
            )"#]],
    )
}

#[test]
fn withdraw_collateral_after_vault_shares_value_increase() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 400
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .withdraw_collateral(VAULT.into(), SENDER.into(), 250)
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 681818181818181818183,
                )),
                BalanceSheet(SetCollateralBalance(
                  vault: "vault",
                  balance: 750,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 81818181818181818182,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 90,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountCollateral(
                  vault: "vault",
                  account: "sender",
                  collateral: 750,
                )),
                BalanceSheet(SetAccountDebt(
                  vault: "vault",
                  account: "sender",
                  debt: 311,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                Vault(Redeem(
                  vault: "vault",
                  shares: "vault_shares_asset",
                  amount: 227272727272727272727,
                  recipient: "sender",
                )),
              ],
              cdp: (
                collateral: 750,
                debt: 311,
                credit: 0,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    )
}

#[test]
fn self_liquidate_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .self_liquidate_position("does_not_exist".into(), SENDER.into())
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn self_liquidate_without_a_position_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .self_liquidate_position(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["nothing to liquidate"],
    )
}

#[test]
fn self_liquidated_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .self_liquidate_position(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn self_liquidate_credit_position() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .self_liquidate_position(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 1,
              )),
              BalanceSheet(SetCollateralBalance(
                vault: "vault",
                balance: 0,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 81818181818181818182,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 90,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountCollateral(
                vault: "vault",
                account: "sender",
                collateral: 0,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 89,
                recipient: "sender",
              )),
              Vault(Redeem(
                vault: "vault",
                shares: "vault_shares_asset",
                amount: 909090909090909090909,
                recipient: "sender",
              )),
            ]"#]],
    )
}

#[test]
fn self_liquidate_debt_position() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .self_liquidate_position(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 0,
              )),
              BalanceSheet(SetCollateralBalance(
                vault: "vault",
                balance: 0,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 500000000000000000000,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 500,
              )),
              BalanceSheet(SetAccountCollateral(
                vault: "vault",
                account: "sender",
                collateral: 0,
              )),
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 0,
              )),
              Vault(Redeem(
                vault: "vault",
                shares: "vault_shares_asset",
                amount: 500000000000000000000,
                recipient: "sender",
              )),
            ]"#]],
    )
}

#[test]
fn self_liquidate_debt_position_after_vault_shares_increase() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .self_liquidate_position(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 2,
              )),
              BalanceSheet(SetCollateralBalance(
                vault: "vault",
                balance: 0,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 455454545454545454545,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 501,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountCollateral(
                vault: "vault",
                account: "sender",
                collateral: 0,
              )),
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 0,
              )),
              Vault(Redeem(
                vault: "vault",
                shares: "vault_shares_asset",
                amount: 535454545454545454545,
                recipient: "sender",
              )),
            ]"#]],
    )
}

#[test]
fn convert_credit_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .convert_credit("does_not_exist".into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn convert_zero_credit_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .convert_credit(VAULT.into(), SENDER.into(), 0)
            .unwrap_err(),
        expect!["cannot convert zero"],
    )
}

#[test]
fn convert_more_credit_than_balance_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .convert_credit(VAULT.into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["not enough credit"],
    )
}

#[test]
fn convert_more_credit_than_in_reserve_errs() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            }
        ]);

    let cmds = hub(&world, &world, &world)
        .redeem_synthetic(
            VAULT.into(),
            SENDER.into(),
            SYNTHETIC.into(),
            50,
            SENDER.into(),
        )
        .unwrap();

    check_err(
        world
            .handle_cmds(cmds)
            .hub()
            .convert_credit(VAULT.into(), SENDER.into(), 89)
            .unwrap_err(),
        expect!["insufficient reserves"],
    )
}

#[test]
fn convert_credit_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .convert_credit(VAULT.into(), SENDER.into(), 1_000)
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn convert_credit() {
    let world = World::default()
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            }
        ])
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000));

    let response = hub(&world, &world, &world)
        .convert_credit(VAULT.into(), SENDER.into(), 89)
        .unwrap();

    check(
        &response,
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 990000000000000000000,
                )),
                BalanceSheet(SetCollateralBalance(
                  vault: "vault",
                  balance: 1088,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 909090909090909092,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 1,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountCollateral(
                  vault: "vault",
                  account: "sender",
                  collateral: 1088,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
              ],
              cdp: (
                collateral: 1088,
                debt: 0,
                credit: 0,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    );

    let world = world.handle_cmds(response.cmds);

    let vault = world.vaults.get(VAULT).unwrap();

    check(
        &vault.balances,
        expect![[r#"
            (
              users: {
                "sender": (
                  collateral: 1088,
                  debt: 0,
                  credit: 0,
                  spr: Some((("0.08999999999999999999999999999999"))),
                ),
              },
              collateral_shares: 990000000000000000000,
              collateral_balance: 1088,
              reserve_shares: 909090909090909092,
              reserve_balance: 1,
              treasury_shares: 9090909090909090908,
              amo_shares: 0,
              spr: Some((("0.08999999999999999999999999999999"))),
            )"#]],
    );

    check(
        vault.balances.collateral_shares
            + vault.balances.reserve_shares
            + vault.balances.treasury_shares
            + vault.balances.amo_shares,
        expect!["1000000000000000000000"],
    );

    check(
        FixedU256::from_u128(vault.balances.collateral_shares)
            .checked_div(FixedU256::from_u128(world.total_issued_shares))
            .unwrap()
            .checked_mul(FixedU256::from_u128(world.total_deposits))
            .unwrap(),
        expect![[r#"("1088.99999999999999999999999999999999")"#]],
    );
}

#[test]
fn redeem_synthetic_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .redeem_synthetic(
                "does_not_exist".into(),
                SENDER.into(),
                SYNTHETIC.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn redeem_zero_synthetic_assets_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                SYNTHETIC.into(),
                0,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["cannot redeem zero"],
    )
}

#[test]
fn redeem_invalid_synthetic_asset_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                "unknown_asset".into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["invalid synthetic asset"],
    )
}

#[test]
fn redeem_not_from_configured_proxy_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetRedeemProxy {
                    vault: VAULT.into(),
                    proxy: "redeem_proxy".into()
                }
            ])
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                SYNTHETIC.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["unauthorized"],
    )
}

#[test]
fn redeem_synthetic_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                SYNTHETIC.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn redeem_synthetic_against_insufficient_reserves_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                SYNTHETIC.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["insufficient reserves"],
    )
}

#[test]
fn redeem_synthetic() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(1_100)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .redeem_synthetic(
                VAULT.into(),
                SENDER.into(),
                SYNTHETIC.into(),
                89,
                SENDER.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 909090909090909090910,
              )),
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 909090909090909092,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 1,
              )),
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 9090909090909090908,
              )),
              BalanceSheet(SetOverallSumPaymentRatio(
                vault: "vault",
                spr: (("0.08999999999999999999999999999999")),
              )),
              BalanceSheet(SetAccountDebt(
                vault: "vault",
                account: "sender",
                debt: 411,
              )),
              BalanceSheet(SetAccountSumPaymentRatio(
                vault: "vault",
                account: "sender",
                spr: (("0.08999999999999999999999999999999")),
              )),
              Vault(Redeem(
                vault: "vault",
                shares: "vault_shares_asset",
                amount: 80909090909090909090,
                recipient: "sender",
              )),
              Mint(Burn(
                synthetic: "synthetic",
                amount: 89,
              )),
            ]"#]],
    );
}

#[test]
fn mint_synthetic_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .mint_synthetic(
                "does_not_exist".into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn mint_zero_synthetic_assets_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                0,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["cannot mint zero"],
    )
}

#[test]
fn mint_not_from_configured_proxy_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetMintProxy {
                    vault: VAULT.into(),
                    proxy: "mint_proxy".into()
                }
            ])
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["deposits disabled"],
    )
}

#[test]
fn mint_synthetic_with_invalid_deposit_asset_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                "unknown_asset".into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["deposits disabled"],
    )
}

#[test]
fn mint_synthetic_while_loss_detected_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["deposits disabled"],
    )
}

#[test]
fn mint_synthetic_when_deposits_disabled_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap_err(),
        expect!["deposits disabled"],
    )
}

#[test]
fn mint_synthetic() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetDepositsEnabled {
                    vault: VAULT.into(),
                    enabled: true
                }
            ])
            .hub()
            .mint_synthetic(
                VAULT.into(),
                SENDER.into(),
                VAULT_DEPOSIT_ASSET.into(),
                1_000,
                SENDER.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              Vault(Deposit(
                vault: "vault",
                asset: "vault_deposit_asset",
                amount: 1000,
                callback_recipient: "sender",
                callback_reason: Mint,
              )),
            ]"#]],
    )
}

#[test]
fn vault_deposit_callback_after_deposit() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .vault_deposit_callback(
                VAULT.into(),
                SENDER.into(),
                VaultDepositReason::Deposit,
                shares_amount(1_000),
                1_000,
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetCollateralShares(
                vault: "vault",
                shares: 1000000000000000000000,
              )),
              BalanceSheet(SetCollateralBalance(
                vault: "vault",
                balance: 1000,
              )),
              BalanceSheet(SetAccountCollateral(
                vault: "vault",
                account: "sender",
                collateral: 1000,
              )),
            ]"#]],
    )
}

#[test]
fn vault_deposit_callback_after_repay_underlying() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .vault_deposit_callback(
                VAULT.into(),
                SENDER.into(),
                VaultDepositReason::RepayUnderlying,
                shares_amount(1_000),
                1_000,
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 1000000000000000000000,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 1000,
              )),
              BalanceSheet(SetAccountCredit(
                vault: "vault",
                account: "sender",
                credit: 1000,
              )),
            ]"#]],
    )
}

#[test]
fn vault_deposit_callback_after_mint() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .vault_deposit_callback(
                VAULT.into(),
                SENDER.into(),
                VaultDepositReason::Mint,
                shares_amount(1_000),
                1_000,
            )
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetReserveShares(
                vault: "vault",
                shares: 1000000000000000000000,
              )),
              BalanceSheet(SetReserveBalance(
                vault: "vault",
                balance: 1000,
              )),
              Mint(Mint(
                synthetic: "synthetic",
                amount: 1000,
                recipient: "sender",
              )),
            ]"#]],
    )
}

#[test]
fn claim_treasury_shares_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .claim_treasury_shares("does_not_exist".into(), SENDER.into())
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn claim_treasury_shares_when_no_treasury_set_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .claim_treasury_shares(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["no treasury set"],
    )
}

#[test]
fn claim_treasury_shares_when_not_the_treasury_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetTreasury {
                    treasury: "treasury".into()
                }
            ])
            .hub()
            .claim_treasury_shares(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["unauthorized"],
    )
}

#[test]
fn claim_treasury_shares_when_nothing_to_claim_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetTreasury {
                    treasury: "treasury".into()
                }
            ])
            .hub()
            .claim_treasury_shares(VAULT.into(), "treasury".into())
            .unwrap_err(),
        expect!["nothing to claim"],
    )
}

#[test]
fn claim_treasury_shares() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetTreasury {
                    treasury: "treasury".into()
                },
                BalanceSheetCmd::SetTreasuryShares {
                    vault: VAULT.into(),
                    shares: shares_amount(10)
                }
            ])
            .hub()
            .claim_treasury_shares(VAULT.into(), "treasury".into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetTreasuryShares(
                vault: "vault",
                shares: 0,
              )),
              BalanceSheet(SendShares(
                shares: "vault_shares_asset",
                amount: 10000000000000000000,
                recipient: "treasury",
              )),
            ]"#]],
    )
}

#[test]
fn claim_amo_shares_unregistered_vault_errs() {
    check_err(
        World::default()
            .hub()
            .claim_amo_shares("does_not_exist".into(), SENDER.into())
            .unwrap_err(),
        expect!["vault not registered"],
    )
}

#[test]
fn claim_amo_shares_when_no_amo_set_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .claim_amo_shares(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["no amo set"],
    )
}

#[test]
fn claim_amo_shares_when_not_the_amo_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAmo {
                    vault: VAULT.into(),
                    amo: "amo".into()
                }
            ])
            .hub()
            .claim_amo_shares(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["unauthorized"],
    )
}

#[test]
fn claim_amo_shares_when_nothing_to_claim_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAmo {
                    vault: VAULT.into(),
                    amo: "amo".into()
                }
            ])
            .hub()
            .claim_amo_shares(VAULT.into(), "amo".into())
            .unwrap_err(),
        expect!["nothing to claim"],
    )
}

#[test]
fn claim_amo_shares() {
    check(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                VaultCmd::SetAmo {
                    vault: VAULT.into(),
                    amo: "amo".into()
                },
                BalanceSheetCmd::SetAmoShares {
                    vault: VAULT.into(),
                    shares: shares_amount(5)
                }
            ])
            .hub()
            .claim_amo_shares(VAULT.into(), "amo".into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetAmoShares(
                vault: "vault",
                shares: 0,
              )),
              BalanceSheet(SendShares(
                shares: "vault_shares_asset",
                amount: 5000000000000000000,
                recipient: "amo",
              )),
            ]"#]],
    )
}

#[test]
fn evaluate_vault_loss_errs() {
    check_err(
        World::default()
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .total_deposits(999)
            .total_shares_issued(shares_amount(1_000))
            .hub()
            .evaluate(VAULT.into(), SENDER.into())
            .unwrap_err(),
        expect!["vault shares have suffered a loss in value"],
    )
}

#[test]
fn evaluate_empty_overall_position() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .hub()
            .evaluate(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [],
              cdp: (
                collateral: 0,
                debt: 0,
                credit: 0,
                spr: (("0.0")),
              ),
            )"#]],
    )
}

#[test]
fn evaluate_open_position_without_change() {
    check(
        World::default()
            .total_deposits(1_000)
            .total_shares_issued(shares_amount(1_000))
            .handle_cmds(cmds![
                VaultCmd::Register {
                    vault: VAULT.into(),
                    synthetic: SYNTHETIC.into()
                },
                BalanceSheetCmd::SetCollateralBalance {
                    vault: VAULT.into(),
                    balance: 1_000
                },
                BalanceSheetCmd::SetCollateralShares {
                    vault: VAULT.into(),
                    shares: shares_amount(1_000)
                },
                BalanceSheetCmd::SetAccountCollateral {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    collateral: 1_000
                },
                BalanceSheetCmd::SetAccountDebt {
                    vault: VAULT.into(),
                    account: SENDER.into(),
                    debt: 500
                }
            ])
            .hub()
            .evaluate(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [],
              cdp: (
                collateral: 1000,
                debt: 500,
                credit: 0,
                spr: (("0.0")),
              ),
            )"#]],
    )
}

#[test]
fn evaluate_open_position_with_debt_after_shares_value_increase() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            },
            BalanceSheetCmd::SetAccountDebt {
                vault: VAULT.into(),
                account: SENDER.into(),
                debt: 500
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        &response,
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 909090909090909090910,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 81818181818181818182,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 90,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountDebt(
                  vault: "vault",
                  account: "sender",
                  debt: 411,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
              ],
              cdp: (
                collateral: 1000,
                debt: 411,
                credit: 0,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    );

    let world = world.handle_cmds(response.cmds);

    let vault = world.vaults.get(VAULT).unwrap();

    check(
        &vault.balances,
        expect![[r#"
            (
              users: {
                "sender": (
                  collateral: 1000,
                  debt: 411,
                  credit: 0,
                  spr: Some((("0.08999999999999999999999999999999"))),
                ),
              },
              collateral_shares: 909090909090909090910,
              collateral_balance: 1000,
              reserve_shares: 81818181818181818182,
              reserve_balance: 90,
              treasury_shares: 9090909090909090908,
              amo_shares: 0,
              spr: Some((("0.08999999999999999999999999999999"))),
            )"#]],
    );

    check(
        vault.balances.collateral_shares
            + vault.balances.reserve_shares
            + vault.balances.treasury_shares
            + vault.balances.amo_shares,
        expect!["1000000000000000000000"],
    );

    check(
        FixedU256::from_u128(vault.balances.collateral_shares)
            .checked_div(FixedU256::from_u128(world.total_issued_shares))
            .unwrap()
            .checked_mul(FixedU256::from_u128(world.total_deposits))
            .unwrap()
            .floor(),
        expect!["1000"],
    );
}

#[test]
fn evaluate_open_position_without_debt_after_shares_value_increase() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        &response,
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 909090909090909090910,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 81818181818181818182,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 90,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountCredit(
                  vault: "vault",
                  account: "sender",
                  credit: 89,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
              ],
              cdp: (
                collateral: 1000,
                debt: 0,
                credit: 89,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    );

    let world = world.handle_cmds(response.cmds);

    let vault = world.vaults.get(VAULT).unwrap();

    check(
        &vault.balances,
        expect![[r#"
            (
              users: {
                "sender": (
                  collateral: 1000,
                  debt: 0,
                  credit: 89,
                  spr: Some((("0.08999999999999999999999999999999"))),
                ),
              },
              collateral_shares: 909090909090909090910,
              collateral_balance: 1000,
              reserve_shares: 81818181818181818182,
              reserve_balance: 90,
              treasury_shares: 9090909090909090908,
              amo_shares: 0,
              spr: Some((("0.08999999999999999999999999999999"))),
            )"#]],
    );

    check(
        vault.balances.collateral_shares
            + vault.balances.reserve_shares
            + vault.balances.treasury_shares
            + vault.balances.amo_shares,
        expect!["1000000000000000000000"],
    );

    check(
        FixedU256::from_u128(vault.balances.collateral_shares)
            .checked_div(FixedU256::from_u128(world.total_issued_shares))
            .unwrap()
            .checked_mul(FixedU256::from_u128(world.total_deposits))
            .unwrap()
            .floor(),
        expect!["1000"],
    );
}

#[test]
fn evaluate_open_position_with_repaid_debt_after_shares_value_increase() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            },
            BalanceSheetCmd::SetAccountDebt {
                vault: VAULT.into(),
                account: SENDER.into(),
                debt: 50
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        &response,
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 909090909090909090910,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 81818181818181818182,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 90,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 9090909090909090908,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.08999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountDebt(
                  vault: "vault",
                  account: "sender",
                  debt: 0,
                )),
                BalanceSheet(SetAccountCredit(
                  vault: "vault",
                  account: "sender",
                  credit: 39,
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "sender",
                  spr: (("0.08999999999999999999999999999999")),
                )),
              ],
              cdp: (
                collateral: 1000,
                debt: 0,
                credit: 39,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    );

    let world = world.handle_cmds(response.cmds);

    let vault = world.vaults.get(VAULT).unwrap();

    check(
        &vault.balances,
        expect![[r#"
            (
              users: {
                "sender": (
                  collateral: 1000,
                  debt: 0,
                  credit: 39,
                  spr: Some((("0.08999999999999999999999999999999"))),
                ),
              },
              collateral_shares: 909090909090909090910,
              collateral_balance: 1000,
              reserve_shares: 81818181818181818182,
              reserve_balance: 90,
              treasury_shares: 9090909090909090908,
              amo_shares: 0,
              spr: Some((("0.08999999999999999999999999999999"))),
            )"#]],
    );

    check(
        vault.balances.collateral_shares
            + vault.balances.reserve_shares
            + vault.balances.treasury_shares
            + vault.balances.amo_shares,
        expect!["1000000000000000000000"],
    );

    check(
        FixedU256::from_u128(vault.balances.collateral_shares)
            .checked_div(FixedU256::from_u128(world.total_issued_shares))
            .unwrap()
            .checked_mul(FixedU256::from_u128(world.total_deposits))
            .unwrap()
            .floor(),
        expect!["1000"],
    );
}

#[test]
fn evaluate_open_position_with_debt_no_change_after_previous_shares_value_increase() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            },
            BalanceSheetCmd::SetAccountDebt {
                vault: VAULT.into(),
                account: SENDER.into(),
                debt: 500
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        world
            .handle_cmds(response.cmds)
            .hub()
            .evaluate(VAULT.into(), SENDER.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [],
              cdp: (
                collateral: 1000,
                debt: 411,
                credit: 0,
                spr: (("0.08999999999999999999999999999999")),
              ),
            )"#]],
    );
}

#[test]
fn evaluate_sender_without_position_after_another_shares_value_increase() {
    let world = World::default()
        .total_deposits(1_100)
        .total_shares_issued(shares_amount(1_000))
        .handle_cmds(cmds![
            VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            },
            BalanceSheetCmd::SetCollateralBalance {
                vault: VAULT.into(),
                balance: 1_000
            },
            BalanceSheetCmd::SetCollateralShares {
                vault: VAULT.into(),
                shares: shares_amount(1_000)
            },
            BalanceSheetCmd::SetAccountCollateral {
                vault: VAULT.into(),
                account: SENDER.into(),
                collateral: 1_000
            },
            BalanceSheetCmd::SetAccountDebt {
                vault: VAULT.into(),
                account: SENDER.into(),
                debt: 500
            }
        ]);

    let response = hub(&world, &world, &world)
        .evaluate(VAULT.into(), SENDER.into())
        .unwrap();

    check(
        world
            .handle_cmds(response.cmds)
            .total_deposits(1_200)
            .hub()
            .evaluate(VAULT.into(), "someone_else".into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                BalanceSheet(SetCollateralShares(
                  vault: "vault",
                  shares: 834090909090909090911,
                )),
                BalanceSheet(SetReserveShares(
                  vault: "vault",
                  shares: 142651515151515151516,
                )),
                BalanceSheet(SetReserveBalance(
                  vault: "vault",
                  balance: 170,
                )),
                BalanceSheet(SetTreasuryShares(
                  vault: "vault",
                  shares: 23257575757575757573,
                )),
                BalanceSheet(SetOverallSumPaymentRatio(
                  vault: "vault",
                  spr: (("0.16999999999999999999999999999999")),
                )),
                BalanceSheet(SetAccountSumPaymentRatio(
                  vault: "vault",
                  account: "someone_else",
                  spr: (("0.16999999999999999999999999999999")),
                )),
              ],
              cdp: (
                collateral: 0,
                debt: 0,
                credit: 0,
                spr: (("0.16999999999999999999999999999999")),
              ),
            )"#]],
    );
}

#[test]
fn register_already_registered_vault_errs() {
    check_err(
        World::default()
            .configure()
            .register_vault(
                AdminRole::mock(),
                ALREADY_REGISTERED_VAULT.into(),
                SYNTHETIC.into(),
            )
            .unwrap_err(),
        expect!["vault already registered"],
    )
}

#[test]
fn register_vault_non_existing_synthetic_errs() {
    check_err(
        World::default()
            .configure()
            .register_vault(AdminRole::mock(), VAULT.into(), "does_not_exist".into())
            .unwrap_err(),
        expect!["synthetic not found"],
    )
}

#[test]
fn register_vault_mismatching_synthetic_decimals_errs() {
    check_err(
        World::default()
            .configure()
            .register_vault(
                AdminRole::mock(),
                VAULT.into(),
                EIGHT_DECIMAL_SYNTHETIC.into(),
            )
            .unwrap_err(),
        expect!["decimals mismatch"],
    )
}

#[test]
fn register_vault() {
    check(
        World::default()
            .configure()
            .register_vault(AdminRole::mock(), VAULT.into(), SYNTHETIC.into())
            .unwrap(),
        expect![[r#"
            [
              Vault(Register(
                vault: "vault",
                synthetic: "synthetic",
              )),
            ]"#]],
    )
}

#[test]
fn set_treasury() {
    check(
        World::default()
            .configure()
            .set_treasury(AdminRole::mock(), "treasury".into())
            .unwrap(),
        expect![[r#"
            [
              BalanceSheet(SetTreasury(
                treasury: "treasury",
              )),
            ]"#]],
    )
}

#[test]
fn set_deposit_enabled() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_deposit_enabled(AdminRole::mock(), VAULT.into(), true)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetDepositsEnabled(
                vault: "vault",
                enabled: true,
              )),
            ]"#]],
    );

    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_deposit_enabled(AdminRole::mock(), VAULT.into(), false)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetDepositsEnabled(
                vault: "vault",
                enabled: false,
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_deposit_enabled(AdminRole::mock(), VAULT.into(), false)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_advance_enabled() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_advance_enabled(AdminRole::mock(), VAULT.into(), true)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAdvanceEnabled(
                vault: "vault",
                enabled: true,
              )),
            ]"#]],
    );

    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_advance_enabled(AdminRole::mock(), VAULT.into(), false)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAdvanceEnabled(
                vault: "vault",
                enabled: false,
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_advance_enabled(AdminRole::mock(), VAULT.into(), false)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_max_ltv() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_max_ltv(AdminRole::mock(), VAULT.into(), 5_000)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetMaxLtv(
                vault: "vault",
                max_ltv: (
                  bps: 5000,
                  rate: (("0.5")),
                ),
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_max_ltv(AdminRole::mock(), VAULT.into(), 10_001)
            .unwrap_err(),
        expect!["invalid rate"],
    );

    check_err(
        World::default()
            .configure()
            .set_max_ltv(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_collateral_yield_fee() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_collateral_yield_fee(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetCollateralYieldFee(
                vault: "vault",
                fee: (
                  bps: 1000,
                  rate: (("0.09999999999999999999999999999999")),
                ),
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_collateral_yield_fee(AdminRole::mock(), VAULT.into(), 10_001)
            .unwrap_err(),
        expect!["invalid rate"],
    );

    check_err(
        World::default()
            .configure()
            .set_collateral_yield_fee(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_reserve_yield_fee() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_reserve_yield_fee(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetReserveYieldFee(
                vault: "vault",
                fee: (
                  bps: 1000,
                  rate: (("0.09999999999999999999999999999999")),
                ),
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_reserve_yield_fee(AdminRole::mock(), VAULT.into(), 10_001)
            .unwrap_err(),
        expect!["invalid rate"],
    );

    check_err(
        World::default()
            .configure()
            .set_reserve_yield_fee(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_advance_fee_recipient() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_advance_fee_recipient(AdminRole::mock(), VAULT.into(), "treasury".into())
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAdvanceFeeRecipient(
                vault: "vault",
                recipient: "treasury",
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_advance_fee_recipient(AdminRole::mock(), VAULT.into(), "treasury".into())
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_fixed_advance_fee() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_fixed_advance_fee(AdminRole::mock(), VAULT.into(), 50)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetFixedAdvanceFee(
                vault: "vault",
                fee: (
                  bps: 50,
                  rate: (("0.00499999999999999999999999999999")),
                ),
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_fixed_advance_fee(AdminRole::mock(), VAULT.into(), 10_001)
            .unwrap_err(),
        expect!["invalid rate"],
    );

    check_err(
        World::default()
            .configure()
            .set_fixed_advance_fee(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_advance_fee_oracle() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_advance_fee_oracle(AdminRole::mock(), VAULT.into(), "oracle".into())
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAdvanceFeeOracle(
                vault: "vault",
                oracle: "oracle",
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_amo(AdminRole::mock(), VAULT.into(), "amo".into())
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_amo() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_amo(AdminRole::mock(), VAULT.into(), "amo".into())
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAmo(
                vault: "vault",
                amo: "amo",
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_amo(AdminRole::mock(), VAULT.into(), "amo".into())
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_amo_allocation() {
    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_amo_allocation(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap(),
        expect![[r#"
            [
              Vault(SetAmoAllocation(
                vault: "vault",
                allocation: (
                  bps: 1000,
                  rate: (("0.09999999999999999999999999999999")),
                ),
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_amo_allocation(AdminRole::mock(), VAULT.into(), 10_001)
            .unwrap_err(),
        expect!["invalid rate"],
    );

    check_err(
        World::default()
            .configure()
            .set_amo_allocation(AdminRole::mock(), VAULT.into(), 1_000)
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

#[test]
fn set_proxy_config() {
    let proxy_config = ProxyConfig {
        deposit: Some("deposit_proxy".into()),
        advance: Some("advance_proxy".into()),
        redeem: Some("redeem_proxy".into()),
        mint: Some("mint_proxy".into()),
    };

    check(
        World::default()
            .handle_cmds(cmds![VaultCmd::Register {
                vault: VAULT.into(),
                synthetic: SYNTHETIC.into()
            }])
            .configure()
            .set_proxy_config(AdminRole::mock(), VAULT.into(), proxy_config.clone())
            .unwrap(),
        expect![[r#"
            [
              Vault(SetDepositProxy(
                vault: "vault",
                proxy: "deposit_proxy",
              )),
              Vault(SetAdvanceProxy(
                vault: "vault",
                proxy: "advance_proxy",
              )),
              Vault(SetRedeemProxy(
                vault: "vault",
                proxy: "redeem_proxy",
              )),
              Vault(SetMintProxy(
                vault: "vault",
                proxy: "mint_proxy",
              )),
            ]"#]],
    );

    check_err(
        World::default()
            .configure()
            .set_proxy_config(AdminRole::mock(), VAULT.into(), proxy_config.clone())
            .unwrap_err(),
        expect!["vault not registered"],
    );
}

impl World {
    fn total_deposits(mut self, deposits: TotalDepositsValue) -> Self {
        self.total_deposits = deposits;
        self
    }

    fn total_shares_issued(mut self, shares: TotalSharesIssued) -> Self {
        self.total_issued_shares = shares;
        self
    }

    fn advance_fee_oracle_rate(mut self, fee: AdvanceFee) -> Self {
        self.oracle_advance_fee = Some(fee);
        self
    }

    fn configure(&self) -> impl ConfigureHub + '_ {
        configure(self, self)
    }

    fn hub(&self) -> impl Hub + '_ {
        hub(self, self, self)
    }

    fn vault_meta_mut(&mut self, vault: Identifier) -> &mut VaultMeta {
        &mut self.vaults.get_mut(vault.as_str()).unwrap().meta
    }

    fn balances_mut(&mut self, vault: Identifier) -> &mut Balances {
        &mut self.vaults.get_mut(vault.as_str()).unwrap().balances
    }

    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Vault(c) => match c {
                VaultCmd::Register { vault, synthetic } => {
                    self.vaults.insert(
                        vault.into_string(),
                        Vault {
                            synthetic,
                            meta: VaultMeta::default(),
                            balances: Balances::default(),
                        },
                    );
                }
                VaultCmd::SetDepositsEnabled { vault, enabled } => {
                    self.vault_meta_mut(vault).deposits_enabled = enabled
                }
                VaultCmd::SetAdvanceEnabled { vault, enabled } => {
                    self.vault_meta_mut(vault).advance_enabled = enabled
                }
                VaultCmd::SetAdvanceFeeOracle { vault, oracle } => {
                    self.vault_meta_mut(vault).advance_fee_oracle = Some(oracle);
                }
                VaultCmd::SetAmo { vault, amo } => {
                    self.vault_meta_mut(vault).amo = Some(amo);
                }
                VaultCmd::SetDepositProxy { vault, proxy } => {
                    self.vault_meta_mut(vault).deposit_proxy = Some(proxy)
                }
                VaultCmd::SetAdvanceProxy { vault, proxy } => {
                    self.vault_meta_mut(vault).advance_proxy = Some(proxy)
                }
                VaultCmd::SetRedeemProxy { vault, proxy } => {
                    self.vault_meta_mut(vault).redeem_proxy = Some(proxy)
                }
                VaultCmd::SetMintProxy { vault, proxy } => {
                    self.vault_meta_mut(vault).mint_proxy = Some(proxy)
                }
                VaultCmd::SetAdvanceFeeRecipient { vault, recipient } => {
                    self.vault_meta_mut(vault).advance_fee_recipient = Some(recipient)
                }
                _ => {}
            },
            Cmd::BalanceSheet(c) => match c {
                BalanceSheetCmd::SetTreasury { treasury } => self.treasury = Some(treasury),
                BalanceSheetCmd::SetCollateralShares { vault, shares } => {
                    self.balances_mut(vault).collateral_shares = shares
                }
                BalanceSheetCmd::SetCollateralBalance { vault, balance } => {
                    self.balances_mut(vault).collateral_balance = balance
                }
                BalanceSheetCmd::SetReserveShares { vault, shares } => {
                    self.balances_mut(vault).reserve_shares = shares
                }
                BalanceSheetCmd::SetReserveBalance { vault, balance } => {
                    self.balances_mut(vault).reserve_balance = balance
                }
                BalanceSheetCmd::SetTreasuryShares { vault, shares } => {
                    self.balances_mut(vault).treasury_shares = shares
                }
                BalanceSheetCmd::SetAmoShares { vault, shares } => {
                    self.balances_mut(vault).amo_shares = shares
                }
                BalanceSheetCmd::SetOverallSumPaymentRatio { vault, spr } => {
                    self.balances_mut(vault).spr = Some(spr)
                }
                BalanceSheetCmd::SetAccountCollateral {
                    vault,
                    account,
                    collateral,
                } => {
                    self.balances_mut(vault)
                        .users
                        .entry(account.into())
                        .or_default()
                        .collateral = collateral
                }
                BalanceSheetCmd::SetAccountDebt {
                    vault,
                    account,
                    debt,
                } => {
                    self.balances_mut(vault)
                        .users
                        .entry(account.into())
                        .or_default()
                        .debt = debt
                }
                BalanceSheetCmd::SetAccountCredit {
                    vault,
                    account,
                    credit,
                } => {
                    self.balances_mut(vault)
                        .users
                        .entry(account.into())
                        .or_default()
                        .credit = credit
                }
                BalanceSheetCmd::SetAccountSumPaymentRatio {
                    vault,
                    account,
                    spr,
                } => {
                    self.balances_mut(vault)
                        .users
                        .entry(account.into())
                        .or_default()
                        .spr = Some(spr)
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_cmds(mut self, cmds: Vec<Cmd>) -> Self {
        for cmd in cmds {
            self.handle_cmd(cmd);
        }
        self
    }
}

impl Vaults for World {
    fn underlying_asset_decimals(&self, vault: &VaultId) -> Option<Decimals> {
        match vault.as_str() {
            VAULT => Some(6),
            _ => None,
        }
    }

    fn is_registered(&self, vault: &VaultId) -> bool {
        if vault.as_str() == ALREADY_REGISTERED_VAULT {
            return true;
        }

        self.vaults.contains_key(vault.as_str())
    }

    fn deposits_enabled(&self, vault: &VaultId) -> bool {
        self.vaults
            .get(vault.as_str())
            .unwrap()
            .meta
            .deposits_enabled
    }

    fn advance_enabled(&self, vault: &VaultId) -> bool {
        self.vaults
            .get(vault.as_str())
            .unwrap()
            .meta
            .advance_enabled
    }

    fn max_ltv(&self, _: &VaultId) -> Option<MaxLtv> {
        None
    }

    fn collateral_yield_fee(&self, _: &VaultId) -> Option<CollateralYieldFee> {
        None
    }

    fn reserve_yield_fee(&self, _: &VaultId) -> Option<ReserveYieldFee> {
        None
    }

    fn fixed_advance_fee(&self, _: &VaultId) -> Option<AdvanceFee> {
        None
    }

    fn advance_fee_recipient(&self, vault: &VaultId) -> Option<Recipient> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.advance_fee_recipient.clone())
    }

    fn advance_fee_oracle(&self, vault: &VaultId) -> Option<Oracle> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.advance_fee_oracle.clone())
    }

    fn amo(&self, vault: &VaultId) -> Option<Amo> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.amo.clone())
    }

    fn amo_allocation(&self, _: &VaultId) -> Option<AmoAllocation> {
        None
    }

    fn deposit_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.deposit_proxy.clone())
    }

    fn advance_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.advance_proxy.clone())
    }

    fn redeem_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.redeem_proxy.clone())
    }

    fn mint_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.meta.mint_proxy.clone())
    }

    fn deposit_asset(&self, vault: &VaultId) -> Asset {
        assert_eq!(vault.as_str(), VAULT);

        VAULT_DEPOSIT_ASSET.into()
    }

    fn shares_asset(&self, vault: &VaultId) -> Asset {
        assert_eq!(vault.as_str(), VAULT);

        VAULT_SHARES_ASSET.into()
    }

    fn synthetic_asset(&self, vault: &VaultId) -> Synthetic {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.synthetic.clone())
            .unwrap()
    }

    fn total_shares_issued(&self, vault: &VaultId) -> TotalSharesIssued {
        assert_eq!(vault.as_str(), VAULT);

        self.total_issued_shares
    }

    fn total_deposits_value(&self, vault: &VaultId) -> TotalDepositsValue {
        assert_eq!(vault.as_str(), VAULT);

        self.total_deposits
    }
}

impl SyntheticMint for World {
    fn syntethic_decimals(&self, synthetic: &Synthetic) -> Option<Decimals> {
        match synthetic.as_str() {
            SYNTHETIC => Some(6),
            EIGHT_DECIMAL_SYNTHETIC => Some(8),
            _ => None,
        }
    }
}

impl BalanceSheet for World {
    fn treasury(&self) -> Option<Treasury> {
        self.treasury.clone()
    }

    fn collateral_shares(&self, vault: &VaultId) -> Option<SharesAmount> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.collateral_shares)
    }

    fn collateral_balance(&self, vault: &VaultId) -> Option<Collateral> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.collateral_balance)
    }

    fn reserve_shares(&self, vault: &VaultId) -> Option<SharesAmount> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.reserve_shares)
    }

    fn reserve_balance(&self, vault: &VaultId) -> Option<Collateral> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.reserve_balance)
    }

    fn treasury_shares(&self, vault: &VaultId) -> Option<TreasuryShares> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.treasury_shares)
    }

    fn amo_shares(&self, vault: &VaultId) -> Option<AmoShares> {
        self.vaults
            .get(vault.as_str())
            .map(|v| v.balances.amo_shares)
    }

    fn overall_sum_payment_ratio(&self, vault: &VaultId) -> Option<SumPaymentRatio> {
        self.vaults.get(vault.as_str()).and_then(|v| v.balances.spr)
    }

    fn account_collateral(&self, vault: &VaultId, account: &Account) -> Option<Collateral> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.balances.users.get(account.as_str()))
            .map(|u| u.collateral)
    }

    fn account_debt(&self, vault: &VaultId, account: &Account) -> Option<Debt> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.balances.users.get(account.as_str()))
            .map(|u| u.debt)
    }

    fn account_credit(&self, vault: &VaultId, account: &Account) -> Option<Credit> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.balances.users.get(account.as_str()))
            .map(|u| u.credit)
    }

    fn account_sum_payment_ratio(
        &self,
        vault: &VaultId,
        account: &Account,
    ) -> Option<SumPaymentRatio> {
        self.vaults
            .get(vault.as_str())
            .and_then(|v| v.balances.users.get(account.as_str()))
            .and_then(|u| u.spr)
    }
}

impl AdvanceFeeOracle for World {
    fn advance_fee(&self, _: &Oracle, _: &Recipient) -> Option<AdvanceFee> {
        self.oracle_advance_fee
    }
}
