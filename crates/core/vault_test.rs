use std::collections::{BTreeMap, HashMap};

use num::FixedU256;
use test_utils::{check, check_err, prelude::expect};

use super::*;

const BOB: &str = "bob";
const ALICE: &str = "alice";
const DEPOSIT_ASSET: &str = "deposit_asset";
const SHARES_ASSET: &str = "shares_asset";

enum UnbondMode {
    Ready,
    Later,
}

#[derive(Default)]
struct WholeBatch {
    value: DepositValue,
    amount: ClaimAmount,
    epoch: Option<UnbondEpoch>,
}

#[derive(Default)]
struct RecipientBatch {
    value: DepositValue,
    next: Option<BatchId>,
}

#[derive(Default)]
struct RecipientEntry {
    first_entered: Option<BatchId>,
    last_entered: Option<BatchId>,
    last_claimed: Option<BatchId>,
    batches: BTreeMap<BatchId, RecipientBatch>,
}

struct World {
    now: u64,
    total_deposits: u128,
    total_shares: u128,
    underlying_redemption_rate: FixedU256,
    unbond_mode: UnbondMode,
    unbonding_batches: BTreeMap<BatchId, WholeBatch>,
    recipient_batches: HashMap<String, RecipientEntry>,
    last_committed_batch_id: Option<BatchId>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            now: 1,
            total_deposits: 0,
            total_shares: 0,
            underlying_redemption_rate: FixedU256::from_u128(1),
            unbond_mode: UnbondMode::Ready,
            unbonding_batches: BTreeMap::default(),
            recipient_batches: HashMap::default(),
            last_committed_batch_id: None,
        }
    }
}

impl World {
    fn vault(&self) -> impl Vault + '_ {
        vault(self, self, self)
    }

    fn now(mut self, v: u64) -> Self {
        self.now = v;
        self
    }

    fn total_deposits(mut self, v: u128) -> Self {
        self.total_deposits = v;
        self
    }

    fn total_shares(mut self, v: u128) -> Self {
        self.total_shares = v;
        self
    }

    fn underlying_redemption_rate(mut self, numer: u128, denom: u128) -> Self {
        self.underlying_redemption_rate = FixedU256::from_u128(numer)
            .checked_div(FixedU256::from_u128(denom))
            .unwrap();
        self
    }

    fn unbond_later(mut self) -> Self {
        self.unbond_mode = UnbondMode::Later;
        self
    }

    fn unbond_ready(mut self) -> Self {
        self.unbond_mode = UnbondMode::Ready;
        self
    }

    fn deposit_amount(&self, DepositValue(value): DepositValue) -> DepositAmount {
        let amount = FixedU256::from_u128(value)
            .checked_div(self.underlying_redemption_rate)
            .unwrap()
            .floor();

        DepositAmount(amount)
    }

    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Mint(c) => match c {
                MintCmd::Mint {
                    amount: SharesAmount(amount),
                    ..
                } => self.total_shares += amount,
                MintCmd::Burn {
                    amount: SharesAmount(amount),
                } => self.total_shares -= amount,
            },
            Cmd::Strategy(c) => match c {
                StrategyCmd::Deposit {
                    amount: DepositAmount(amount),
                } => self.total_deposits += amount,
                StrategyCmd::Unbond { value } => {
                    self.total_deposits -= self.deposit_amount(value).0
                }
                _ => {}
            },
            Cmd::UnbondingLog(c) => match c {
                UnbondingLogSet::LastCommittedBatchId(id) => {
                    self.last_committed_batch_id = Some(id)
                }
                UnbondingLogSet::BatchTotalUnbondValue { batch, value } => {
                    self.unbonding_batches.entry(batch).or_default().value = value;
                }
                UnbondingLogSet::BatchClaimableAmount { batch, amount } => {
                    self.unbonding_batches.entry(batch).or_default().amount = amount;
                }
                UnbondingLogSet::BatchEpoch { batch, epoch } => {
                    self.unbonding_batches.entry(batch).or_default().epoch = Some(epoch);
                }
                UnbondingLogSet::FirstEnteredBatch { recipient, batch } => {
                    self.recipient_batches
                        .entry(recipient.into_string())
                        .or_default()
                        .first_entered = Some(batch);
                }
                UnbondingLogSet::LastEnteredBatch { recipient, batch } => {
                    self.recipient_batches
                        .entry(recipient.into_string())
                        .or_default()
                        .last_entered = Some(batch);
                }
                UnbondingLogSet::NextEnteredBatch {
                    recipient,
                    previous,
                    next,
                } => {
                    self.recipient_batches
                        .entry(recipient.into_string())
                        .or_default()
                        .batches
                        .get_mut(&previous)
                        .unwrap()
                        .next = Some(next);
                }
                UnbondingLogSet::LastClaimedBatch { recipient, batch } => {
                    self.recipient_batches
                        .entry(recipient.into_string())
                        .or_default()
                        .last_claimed = Some(batch)
                }
                UnbondingLogSet::UnbondedValueInBatch {
                    recipient,
                    batch,
                    value: DepositValue(value),
                } => {
                    let entry = self
                        .recipient_batches
                        .entry(recipient.into_string())
                        .or_default()
                        .batches
                        .entry(batch)
                        .or_default();

                    entry.value = DepositValue(value + entry.value.0);
                }
                _ => {}
            },
        }
    }

    fn handle_cmds(mut self, cmds: Vec<Cmd>) -> Self {
        for cmd in cmds {
            self.handle_cmd(cmd);
        }
        self
    }
}

const fn shares_amount(n: u128) -> u128 {
    n * 10u128.pow(SHARES_DECIMAL_PLACES)
}

#[test]
fn deposit_zero_errs() {
    check_err(
        World::default()
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(0), BOB.into())
            .unwrap_err(),
        expect!["cannot deposit zero"],
    )
}

#[test]
fn deposit_invalid_asset_errs() {
    check_err(
        World::default()
            .vault()
            .deposit("invalid_asset".into(), DepositAmount(100), BOB.into())
            .unwrap_err(),
        expect!["invalid deposit asset"],
    )
}

#[test]
fn deposit_causing_overflow_errs() {
    check_err(
        World::default()
            .total_deposits(u128::MAX)
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(1), BOB.into())
            .unwrap_err(),
        expect!["deposit too large"],
    )
}

#[test]
fn deposit_too_small_errs() {
    check_err(
        World::default()
            .total_deposits(shares_amount(1) + 1)
            .total_shares(shares_amount(1))
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(1), BOB.into())
            .unwrap_err(),
        expect!["deposit too small"],
    )
}

#[test]
fn deposit_after_total_loss_errs() {
    check_err(
        World::default()
            .total_shares(10)
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(1), BOB.into())
            .unwrap_err(),
        expect!["cannot deposit in total loss state"],
    )
}

#[test]
fn initial_deposit() {
    check(
        World::default()
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(100), BOB.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                Strategy(Deposit(
                  amount: (100),
                )),
                Mint(Mint(
                  amount: (100000000000000),
                  recipient: "bob",
                )),
              ],
              deposit_value: (100),
              issued_shares: (100000000000000),
              total_shares_issued: (100000000000000),
              total_deposits_value: (100),
            )"#]],
    )
}

#[test]
fn initial_deposit_with_underlying_redemption_rate_gt_1() {
    check(
        World::default()
            .underlying_redemption_rate(11, 10)
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(100), BOB.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                Strategy(Deposit(
                  amount: (100),
                )),
                Mint(Mint(
                  amount: (109000000000000),
                  recipient: "bob",
                )),
              ],
              deposit_value: (109),
              issued_shares: (109000000000000),
              total_shares_issued: (109000000000000),
              total_deposits_value: (109),
            )"#]],
    )
}

#[test]
fn regular_deposit() {
    check(
        World::default()
            .total_deposits(1_200)
            .total_shares(shares_amount(1_000))
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(100), BOB.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                Strategy(Deposit(
                  amount: (100),
                )),
                Mint(Mint(
                  amount: (83333333333333333333),
                  recipient: "bob",
                )),
              ],
              deposit_value: (99),
              issued_shares: (83333333333333333333),
              total_shares_issued: (1083333333333333333333),
              total_deposits_value: (1300),
            )"#]],
    )
}

#[test]
fn regular_deposit_with_underlying_redemption_rate_gt_1() {
    check(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .underlying_redemption_rate(11, 10)
            .vault()
            .deposit(DEPOSIT_ASSET.into(), DepositAmount(100), BOB.into())
            .unwrap(),
        expect![[r#"
            (
              cmds: [
                Strategy(Deposit(
                  amount: (100),
                )),
                Mint(Mint(
                  amount: (99181073703366696997),
                  recipient: "bob",
                )),
              ],
              deposit_value: (108),
              issued_shares: (99181073703366696997),
              total_shares_issued: (1099181073703366696997),
              total_deposits_value: (1208),
            )"#]],
    )
}

#[test]
fn donate_zero_errs() {
    check_err(
        World::default()
            .vault()
            .donate(DEPOSIT_ASSET.into(), DepositAmount(0))
            .unwrap_err(),
        expect!["cannot donate zero"],
    )
}

#[test]
fn donate_invalid_asset_errs() {
    check_err(
        World::default()
            .vault()
            .donate("invalid_asset".into(), DepositAmount(100))
            .unwrap_err(),
        expect!["invalid donation asset"],
    )
}

#[test]
fn donate() {
    check(
        World::default()
            .vault()
            .donate(DEPOSIT_ASSET.into(), DepositAmount(100))
            .unwrap(),
        expect![[r#"
            Deposit(
              amount: (100),
            )"#]],
    )
}

#[test]
fn redeem_invalid_asset_errs() {
    check_err(
        World::default()
            .vault()
            .redeem("invalid_asset".into(), SharesAmount(1), BOB.into())
            .unwrap_err(),
        expect!["invalid redemption asset"],
    )
}

#[test]
fn redeem_zero_errs() {
    check_err(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .vault()
            .redeem(SHARES_ASSET.into(), SharesAmount(0), BOB.into())
            .unwrap_err(),
        expect!["cannot redeem zero"],
    )
}

#[test]
fn redeem_total_loss_errs() {
    check_err(
        World::default()
            .total_shares(shares_amount(1_000))
            .vault()
            .redeem(
                SHARES_ASSET.into(),
                SharesAmount(shares_amount(100)),
                BOB.into(),
            )
            .unwrap_err(),
        expect!["no deposits to redeem"],
    )
}

#[test]
fn redeem_too_little_errs() {
    check_err(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .vault()
            .redeem(SHARES_ASSET.into(), SharesAmount(1), BOB.into())
            .unwrap_err(),
        expect!["redemption too small"],
    )
}

#[test]
fn redeem_first_time_unbond_ready() {
    check(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .vault()
            .redeem(
                SHARES_ASSET.into(),
                SharesAmount(shares_amount(100)),
                BOB.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              UnbondingLog(BatchTotalUnbondValue(
                batch: 0,
                value: (100),
              )),
              UnbondingLog(UnbondedValueInBatch(
                recipient: "bob",
                batch: 0,
                value: (100),
              )),
              Mint(Burn(
                amount: (100000000000000000000),
              )),
              UnbondingLog(LastEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(FirstEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(LastCommittedBatchId(0)),
              UnbondingLog(BatchClaimableAmount(
                batch: 0,
                amount: (100),
              )),
              UnbondingLog(BatchEpoch(
                batch: 0,
                epoch: (
                  start: 1,
                  end: 2,
                ),
              )),
              Strategy(Unbond(
                value: (100),
              )),
            ]"#]],
    )
}

#[test]
fn redeem_first_time_unbond_ready_underlying_redemption_rate_gt_1() {
    check(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .underlying_redemption_rate(11, 10)
            .vault()
            .redeem(
                SHARES_ASSET.into(),
                SharesAmount(shares_amount(100)),
                BOB.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              UnbondingLog(BatchTotalUnbondValue(
                batch: 0,
                value: (109),
              )),
              UnbondingLog(UnbondedValueInBatch(
                recipient: "bob",
                batch: 0,
                value: (109),
              )),
              Mint(Burn(
                amount: (100000000000000000000),
              )),
              UnbondingLog(LastEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(FirstEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(LastCommittedBatchId(0)),
              UnbondingLog(BatchClaimableAmount(
                batch: 0,
                amount: (99),
              )),
              UnbondingLog(BatchEpoch(
                batch: 0,
                epoch: (
                  start: 1,
                  end: 2,
                ),
              )),
              Strategy(Unbond(
                value: (109),
              )),
            ]"#]],
    )
}

#[test]
fn redeem_first_time_unbond_later() {
    check(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .unbond_later()
            .vault()
            .redeem(
                SHARES_ASSET.into(),
                SharesAmount(shares_amount(100)),
                BOB.into(),
            )
            .unwrap(),
        expect![[r#"
            [
              UnbondingLog(BatchTotalUnbondValue(
                batch: 0,
                value: (100),
              )),
              UnbondingLog(UnbondedValueInBatch(
                recipient: "bob",
                batch: 0,
                value: (100),
              )),
              Mint(Burn(
                amount: (100000000000000000000),
              )),
              UnbondingLog(LastEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(FirstEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(BatchHint(
                batch: 0,
                hint: 1,
              )),
            ]"#]],
    )
}

#[test]
fn multiple_redemptions_in_one_batch() {
    const INITIAL_DEPOSITS: u128 = 1000;
    const BOB_REDEEM: u128 = 100;
    const ALICE_REDEEM: u128 = 100;

    let world = World::default()
        .total_deposits(INITIAL_DEPOSITS)
        .total_shares(shares_amount(INITIAL_DEPOSITS))
        .unbond_later();

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(BOB_REDEEM)),
            BOB.into(),
        )
        .unwrap();

    check(
        &cmds,
        expect![[r#"
            [
              UnbondingLog(BatchTotalUnbondValue(
                batch: 0,
                value: (100),
              )),
              UnbondingLog(UnbondedValueInBatch(
                recipient: "bob",
                batch: 0,
                value: (100),
              )),
              Mint(Burn(
                amount: (100000000000000000000),
              )),
              UnbondingLog(LastEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(FirstEnteredBatch(
                recipient: "bob",
                batch: 0,
              )),
              UnbondingLog(BatchHint(
                batch: 0,
                hint: 1,
              )),
            ]"#]],
    );

    let world = world.handle_cmds(cmds).unbond_ready();

    let cmds = world
        .vault()
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(ALICE_REDEEM)),
            ALICE.into(),
        )
        .unwrap();

    check(
        &cmds,
        expect![[r#"
            [
              UnbondingLog(BatchTotalUnbondValue(
                batch: 0,
                value: (200),
              )),
              UnbondingLog(UnbondedValueInBatch(
                recipient: "alice",
                batch: 0,
                value: (100),
              )),
              Mint(Burn(
                amount: (100000000000000000000),
              )),
              UnbondingLog(LastEnteredBatch(
                recipient: "alice",
                batch: 0,
              )),
              UnbondingLog(FirstEnteredBatch(
                recipient: "alice",
                batch: 0,
              )),
              UnbondingLog(LastCommittedBatchId(0)),
              UnbondingLog(BatchClaimableAmount(
                batch: 0,
                amount: (200),
              )),
              UnbondingLog(BatchEpoch(
                batch: 0,
                epoch: (
                  start: 1,
                  end: 2,
                ),
              )),
              Strategy(Unbond(
                value: (200),
              )),
            ]"#]],
    );

    let world = world.handle_cmds(cmds);

    assert_eq!(
        world.total_deposits,
        INITIAL_DEPOSITS - BOB_REDEEM - ALICE_REDEEM
    )
}

#[test]
fn redeem_in_multiple_batches() {
    let world = World::default()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    let cmds = world
        .handle_cmds(cmds)
        .vault()
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    let next_entered_cmd = cmds
        .into_iter()
        .find(|cmd| {
            matches!(
                cmd,
                Cmd::UnbondingLog(UnbondingLogSet::NextEnteredBatch { .. })
            )
        })
        .unwrap();

    check(
        next_entered_cmd,
        expect![[r#"
            UnbondingLog(NextEnteredBatch(
              recipient: "bob",
              previous: 0,
              next: 1,
            ))"#]],
    );
}

#[test]
fn claim_without_unbonding_batches_errs() {
    check_err(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .vault()
            .claim(BOB.into())
            .unwrap_err(),
        expect!["nothing to claim"],
    )
}

#[test]
fn claim_without_finished_batches_errs() {
    let world = World::default()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    check_err(
        world
            .handle_cmds(cmds)
            .vault()
            .claim(BOB.into())
            .unwrap_err(),
        expect!["nothing to claim"],
    )
}

#[test]
fn claim() {
    let world = World::default()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    check(
        world
            .handle_cmds(cmds)
            .now(2)
            .vault()
            .claim(BOB.into())
            .unwrap(),
        expect![[r#"
            [
              UnbondingLog(LastClaimedBatch(
                recipient: "bob",
                batch: 0,
              )),
              Strategy(SendClaimed(
                amount: (100),
                recipient: "bob",
              )),
            ]"#]],
    )
}

#[test]
fn claim_multiple() {
    let world = World::default()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    let world = world.handle_cmds(cmds).now(2);

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(200)),
            BOB.into(),
        )
        .unwrap();

    let world = world.handle_cmds(cmds).now(3);

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(500)),
            BOB.into(),
        )
        .unwrap();

    check(
        world.handle_cmds(cmds).vault().claim(BOB.into()).unwrap(),
        expect![[r#"
            [
              UnbondingLog(LastClaimedBatch(
                recipient: "bob",
                batch: 1,
              )),
              Strategy(SendClaimed(
                amount: (300),
                recipient: "bob",
              )),
            ]"#]],
    )
}

#[test]
fn start_unbond_with_empty_ready_pending_batch_errs() {
    check_err(
        World::default()
            .total_deposits(1_000)
            .total_shares(shares_amount(1_000))
            .vault()
            .start_unbond()
            .unwrap_err(),
        expect!["nothing to unbond"],
    )
}

#[test]
fn start_unbond_with_still_pending_batch_errs() {
    let world = World::default()
        .unbond_later()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    check_err(
        world.handle_cmds(cmds).vault().start_unbond().unwrap_err(),
        expect!["unbond not ready"],
    )
}

#[test]
fn start_unbond() {
    let world = World::default()
        .unbond_later()
        .total_deposits(1_000)
        .total_shares(shares_amount(1_000));

    let cmds = vault(&world, &world, &world)
        .redeem(
            SHARES_ASSET.into(),
            SharesAmount(shares_amount(100)),
            BOB.into(),
        )
        .unwrap();

    check(
        world
            .handle_cmds(cmds)
            .unbond_ready()
            .vault()
            .start_unbond()
            .unwrap(),
        expect![[r#"
            [
              UnbondingLog(LastCommittedBatchId(0)),
              UnbondingLog(BatchClaimableAmount(
                batch: 0,
                amount: (100),
              )),
              UnbondingLog(BatchEpoch(
                batch: 0,
                epoch: (
                  start: 1,
                  end: 2,
                ),
              )),
              Strategy(Unbond(
                value: (100),
              )),
            ]"#]],
    )
}

impl serde::Serialize for crate::Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_str().serialize(serializer)
    }
}

impl Strategy for World {
    fn now(&self) -> Now {
        self.now
    }

    fn deposit_asset(&self) -> Asset {
        DEPOSIT_ASSET.into()
    }

    fn underlying_asset_decimals(&self) -> Decimals {
        6
    }

    fn total_deposits_value(&self) -> TotalDepositsValue {
        let DepositValue(total_deposit_value) =
            self.deposit_value(DepositAmount(self.total_deposits));
        TotalDepositsValue(total_deposit_value)
    }

    fn deposit_value(&self, DepositAmount(amount): DepositAmount) -> DepositValue {
        let value = self
            .underlying_redemption_rate
            .checked_mul(FixedU256::from_u128(amount))
            .unwrap()
            .floor();

        DepositValue(value)
    }

    fn unbond(&self, value: DepositValue) -> UnbondReadyStatus {
        match self.unbond_mode {
            UnbondMode::Ready => {
                let DepositAmount(amount) = self.deposit_amount(value);

                let epoch = UnbondEpoch {
                    start: self.now(),
                    end: self.now() + 1,
                };

                UnbondReadyStatus::Ready {
                    amount: ClaimAmount(amount),
                    epoch,
                }
            }

            UnbondMode::Later => UnbondReadyStatus::Later(Some(1)),
        }
    }
}

impl UnbondingLog for World {
    fn last_committed_batch_id(&self) -> Option<BatchId> {
        self.last_committed_batch_id
    }

    fn batch_unbond_value(&self, batch: BatchId) -> Option<DepositValue> {
        self.unbonding_batches.get(&batch).map(|b| b.value)
    }

    fn batch_claimable_amount(&self, batch: BatchId) -> Option<ClaimAmount> {
        self.unbonding_batches.get(&batch).map(|b| b.amount)
    }

    fn pending_batch_hint(&self, _batch: BatchId) -> Option<Hint> {
        unimplemented!("only included for downstream use")
    }

    fn committed_batch_epoch(&self, batch: BatchId) -> Option<UnbondEpoch> {
        self.unbonding_batches.get(&batch).and_then(|b| b.epoch)
    }

    fn first_entered_batch(&self, recipient: &str) -> Option<BatchId> {
        self.recipient_batches
            .get(recipient)
            .and_then(|r| r.first_entered)
    }

    fn last_entered_batch(&self, recipient: &str) -> Option<BatchId> {
        self.recipient_batches
            .get(recipient)
            .and_then(|e| e.last_entered)
    }

    fn next_entered_batch(&self, recipient: &str, batch: BatchId) -> Option<BatchId> {
        self.recipient_batches
            .get(recipient)
            .and_then(|r| r.batches.get(&batch))
            .and_then(|b| b.next)
    }

    fn last_claimed_batch(&self, recipient: &str) -> Option<BatchId> {
        self.recipient_batches
            .get(recipient)
            .and_then(|r| r.last_claimed)
    }

    fn unbonded_value_in_batch(&self, recipient: &str, batch: BatchId) -> Option<DepositValue> {
        self.recipient_batches
            .get(recipient)
            .and_then(|e| e.batches.get(&batch))
            .map(|b| b.value)
    }
}

impl SharesMint for World {
    fn total_shares_issued(&self) -> TotalSharesIssued {
        TotalSharesIssued(self.total_shares)
    }

    fn shares_asset(&self) -> Asset {
        SHARES_ASSET.into()
    }
}
