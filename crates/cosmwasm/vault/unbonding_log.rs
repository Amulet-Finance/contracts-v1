use amulet_core::vault::{
    BatchId, ClaimAmount, DepositValue, Hint, UnbondEpoch, UnbondingLog as CoreUnbondingLog,
    UnbondingLogSet,
};
use cosmwasm_std::Storage;

use crate::StorageExt as _;

pub struct UnbondingLog<'a>(&'a dyn Storage);

impl<'a> UnbondingLog<'a> {
    pub fn new(storage: &'a dyn Storage) -> Self {
        Self(storage)
    }
}

#[rustfmt::skip]
mod key {
    use crate::MapKey;
    
    macro_rules! key {
        ($k:literal) => {
            concat!("unbonding_log::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            crate::MapKey::new(key!($k))
        };
    }

    pub const LAST_COMMITTED_BATCH_ID     : &str   = key!("last_committed_batch_id");
    pub const BATCH_UNBOND_VALUE          : MapKey = map_key!("batch_unbond_value");
    pub const BATCH_CLAIMABLE_AMOUNT      : MapKey = map_key!("batch_claimable_amount");
    pub const BATCH_HINT                  : MapKey = map_key!("pending_batch_hint");
    pub const COMMITTED_BATCH_EPOCH_START : MapKey = map_key!("committed_batch_epoch_start");
    pub const COMMITTED_BATCH_EPOCH_END   : MapKey = map_key!("committed_batch_epoch_end");
    pub const FIRST_ENTERED_BATCH         : MapKey = map_key!("first_entered_batch");
    pub const LAST_ENTERED_BATCH          : MapKey = map_key!("last_entered_batch");
    pub const NEXT_ENTERED_BATCH          : MapKey = map_key!("next_entered_batch");
    pub const PREVIOUS_ENTERED_BATCH      : MapKey = map_key!("previous_entered_batch");
    pub const LAST_CLAIMED_BATCH          : MapKey = map_key!("last_claimed_batch");
    pub const UNBONDED_VALUE_IN_BATCH     : MapKey = map_key!("unbonded_value_in_batch");
}

pub trait StorageExt: Storage {
    fn previously_entered_batch(&self, recipient: &str, batch: BatchId) -> Option<BatchId> {
        self.u64_at(key::PREVIOUS_ENTERED_BATCH.multi([&recipient, &batch]))
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

impl<'a> CoreUnbondingLog for UnbondingLog<'a> {
    fn last_committed_batch_id(&self) -> Option<BatchId> {
        self.0.u64_at(key::LAST_COMMITTED_BATCH_ID)
    }

    fn batch_unbond_value(&self, batch: BatchId) -> Option<DepositValue> {
        self.0.u128_at(key::BATCH_UNBOND_VALUE.with(batch))
    }

    fn batch_claimable_amount(&self, batch: BatchId) -> Option<ClaimAmount> {
        self.0.u128_at(key::BATCH_CLAIMABLE_AMOUNT.with(batch))
    }

    fn pending_batch_hint(&self, batch: BatchId) -> Option<Hint> {
        self.0.u64_at(key::BATCH_HINT.with(batch))
    }

    fn committed_batch_epoch(&self, batch: BatchId) -> Option<UnbondEpoch> {
        let start = self.0.u64_at(key::COMMITTED_BATCH_EPOCH_START.with(batch));

        let end = self.0.u64_at(key::COMMITTED_BATCH_EPOCH_END.with(batch));

        start
            .zip(end)
            .map(|(start, end)| UnbondEpoch { start, end })
    }

    fn first_entered_batch(&self, recipient: &str) -> Option<BatchId> {
        self.0.u64_at(key::FIRST_ENTERED_BATCH.with(recipient))
    }

    fn last_entered_batch(&self, recipient: &str) -> Option<BatchId> {
        self.0.u64_at(key::LAST_ENTERED_BATCH.with(recipient))
    }

    fn next_entered_batch(&self, recipient: &str, batch: BatchId) -> Option<BatchId> {
        self.0
            .u64_at(key::NEXT_ENTERED_BATCH.multi([&recipient, &batch]))
    }

    fn last_claimed_batch(&self, recipient: &str) -> Option<BatchId> {
        self.0.u64_at(key::LAST_CLAIMED_BATCH.with(recipient))
    }

    fn unbonded_value_in_batch(&self, recipient: &str, batch: BatchId) -> Option<DepositValue> {
        self.0
            .u128_at(key::UNBONDED_VALUE_IN_BATCH.multi([&recipient, &batch]))
    }
}

pub fn handle_cmd(storage: &mut dyn Storage, cmd: UnbondingLogSet) {
    match cmd {
        UnbondingLogSet::LastCommittedBatchId(batch_id) => {
            storage.set_u64(key::LAST_COMMITTED_BATCH_ID, batch_id)
        }

        UnbondingLogSet::BatchTotalUnbondValue { batch, value } => {
            storage.set_u128(key::BATCH_UNBOND_VALUE.with(batch), value)
        }

        UnbondingLogSet::BatchClaimableAmount { batch, amount } => {
            storage.set_u128(key::BATCH_CLAIMABLE_AMOUNT.with(batch), amount)
        }

        UnbondingLogSet::BatchHint { batch, hint } => {
            storage.set_u64(key::BATCH_HINT.with(batch), hint)
        }

        UnbondingLogSet::BatchEpoch { batch, epoch } => {
            storage.set_u64(key::COMMITTED_BATCH_EPOCH_START.with(batch), epoch.start);
            storage.set_u64(key::COMMITTED_BATCH_EPOCH_END.with(batch), epoch.end);
        }

        UnbondingLogSet::FirstEnteredBatch { recipient, batch } => {
            storage.set_u64(key::FIRST_ENTERED_BATCH.with(recipient), batch);
        }

        UnbondingLogSet::LastEnteredBatch { recipient, batch } => {
            storage.set_u64(key::LAST_ENTERED_BATCH.with(recipient), batch);
        }

        UnbondingLogSet::NextEnteredBatch {
            recipient,
            previous,
            next,
        } => {
            storage.set_u64(key::NEXT_ENTERED_BATCH.multi([&recipient, &previous]), next);
            // also link batches in the opposite direction in order to allow descending iteration
            storage.set_u64(
                key::PREVIOUS_ENTERED_BATCH.multi([&recipient, &next]),
                previous,
            );
        }

        UnbondingLogSet::LastClaimedBatch { recipient, batch } => {
            storage.set_u64(key::LAST_CLAIMED_BATCH.with(recipient), batch);
        }

        UnbondingLogSet::UnbondedValueInBatch {
            recipient,
            batch,
            value,
        } => {
            storage.set_u128(
                key::UNBONDED_VALUE_IN_BATCH.multi([&recipient, &batch]),
                value,
            );
        }
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::testing::MockStorage;

    use super::*;

    #[test]
    fn unbonding_log_core_impl() {
        let mut storage = MockStorage::new();

        handle_cmd(&mut storage, UnbondingLogSet::LastCommittedBatchId(1));

        assert_eq!(
            UnbondingLog::new(&storage).last_committed_batch_id(),
            Some(1)
        );
    }
}
