use amulet_core::vault::BatchId;
use amulet_cw::StorageExt as _;
use amulet_token_factory::Flavour as TokenFactoryFlavour;
use cosmwasm_std::Storage;

pub type ValidatorSet = rustc_hash::FxHashSet<String>;

#[rustfmt::skip]
mod key {
    use amulet_cw::MapKey;
    
    macro_rules! key {
        ($k:literal) => {
            concat!("native_pos::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            amulet_cw::MapKey::new(key!($k))
        };
    }

    pub const AVAILABLE_TO_CLAIM: &str          = key!("available_to_claim");
    pub const BATCH_ADJUSTED_CLAIMABLE: MapKey  = map_key!("batch_adjusted_claimable");
    pub const BOND_DENOM: &str                  = key!("bond_denom");
    pub const BOND_DENOM_DECIMALS: &str         = key!("bond_denom_decimals");
    pub const DELEGATED: &str                   = key!("delegated");
    pub const LAST_ACKNOWLEDGED_BATCH: &str     = key!("last_acknowledged_batch");
    pub const LAST_UNBOND_TIMESTAMP: &str       = key!("last_unbond_timestamp");
    pub const MAX_UNBONDING_ENTRIES: &str       = key!("max_unbonding_entries");
    pub const MINIMUM_UNBOND_INTERVAL: &str     = key!("minimum_unbond_interval");
    pub const PENDING_REDELEGATE: MapKey        = map_key!("pending_redelegate");
    pub const PENDING_REDELEGATE_SLOT: MapKey   = map_key!("pending_redelegate_slot");
    pub const PENDING_REDELEGATE_SET_SIZE: &str = key!("pending_redelegate_set_size");
    pub const REWARDS_SINK_ADDRESS: &str        = key!("rewards_sink_address");
    pub const TOKEN_FACTORY_FLAVOUR: &str       = key!("token_factory_flavour");
    pub const UNBONDING_PERIOD: &str            = key!("unbonding_period");
    pub const VALIDATOR: MapKey                 = map_key!("validator");
    pub const VALIDATOR_SLOT: MapKey            = map_key!("validator_slot");
    pub const VALIDATOR_SET_SIZE: &str          = key!("validator_set_size");
}

pub trait StorageExt: Storage {
    fn available_to_claim(&self) -> u128 {
        self.u128_at(key::AVAILABLE_TO_CLAIM).unwrap_or_default()
    }

    fn set_available_to_claim(&mut self, amount: u128) {
        self.set_u128(key::AVAILABLE_TO_CLAIM, amount)
    }

    fn batch_adjusted_claimable(&self, batch: BatchId) -> Option<u128> {
        self.u128_at(key::BATCH_ADJUSTED_CLAIMABLE.with(batch))
    }

    fn set_batch_adjusted_claimable(&mut self, batch: BatchId, amount: u128) {
        self.set_u128(key::BATCH_ADJUSTED_CLAIMABLE.with(batch), amount)
    }

    fn bond_denom(&self) -> String {
        self.string_at(key::BOND_DENOM)
            .expect("set during initialisation")
    }

    fn set_bond_denom(&mut self, bond_denom: &str) {
        self.set_string(key::BOND_DENOM, bond_denom);
    }

    fn bond_denom_decimals(&self) -> u32 {
        self.u32_at(key::BOND_DENOM_DECIMALS)
            .expect("set during initialisation")
    }

    fn set_bond_denom_decimals(&mut self, bond_denom_decimals: u32) {
        self.set_u32(key::BOND_DENOM_DECIMALS, bond_denom_decimals);
    }

    fn delegated(&self) -> u128 {
        self.u128_at(key::DELEGATED).unwrap_or_default()
    }

    fn set_delegated(&mut self, delegated: u128) {
        self.set_u128(key::DELEGATED, delegated)
    }

    fn last_acknowledged_batch(&self) -> Option<BatchId> {
        self.u64_at(key::LAST_ACKNOWLEDGED_BATCH)
    }

    fn set_last_acknowledged_batch(&mut self, last_acknowledged_batch: BatchId) {
        self.set_u64(key::LAST_ACKNOWLEDGED_BATCH, last_acknowledged_batch);
    }

    fn last_unbond_timestamp(&self) -> u64 {
        self.u64_at(key::LAST_UNBOND_TIMESTAMP)
            .expect("set during initialisation")
    }

    fn set_last_unbond_timestamp(&mut self, last_unbond_timestamp: u64) {
        self.set_u64(key::LAST_UNBOND_TIMESTAMP, last_unbond_timestamp);
    }

    fn max_unbonding_entries(&self) -> u64 {
        self.u64_at(key::MAX_UNBONDING_ENTRIES)
            .expect("set during initialisation")
    }

    fn set_max_unbonding_entries(&mut self, max_unbonding_entries: u64) {
        self.set_u64(key::MAX_UNBONDING_ENTRIES, max_unbonding_entries);
    }

    fn minimum_unbond_interval(&self) -> u64 {
        self.u64_at(key::MINIMUM_UNBOND_INTERVAL)
            .expect("set during initialisation")
    }

    fn set_minimum_unbond_interval(&mut self, minimum_unbond_interval: u64) {
        self.set_u64(key::MINIMUM_UNBOND_INTERVAL, minimum_unbond_interval);
    }

    fn validator_pending_redelegation(&self, slot_idx: usize) -> Option<String> {
        self.string_at(key::PENDING_REDELEGATE.with(slot_idx))
    }

    fn validators_pending_redelegation(&self) -> ValidatorSet {
        let set_size = self.validators_pending_redelegation_set_size();

        let mut validators_pending_redelegations = ValidatorSet::default();

        for slot_idx in 0..set_size {
            validators_pending_redelegations.insert(
                self.validator_pending_redelegation(slot_idx)
                    .expect("validator pending redelegation always set for slot index < set size"),
            );
        }

        validators_pending_redelegations
    }

    fn set_validator_pending_redelegation(
        &mut self,
        slot_idx: usize,
        validator_pending_redelegation: &str,
    ) {
        self.set_string(
            key::PENDING_REDELEGATE.with(slot_idx),
            validator_pending_redelegation,
        )
    }

    fn add_validator_pending_redelegation(&mut self, validator_pending_redelegation: &str) {
        let set_size = self.validators_pending_redelegation_set_size();

        if self
            .validator_pending_redelegation_slot(validator_pending_redelegation)
            .is_some()
        {
            panic!(
                "validator pending redelegation already exists: {validator_pending_redelegation}"
            );
        }

        self.set_validator_pending_redelegation(set_size, validator_pending_redelegation);
        self.set_validator_pending_redelegation_slot(validator_pending_redelegation, set_size);
        self.set_validators_pending_redelegation_set_size(set_size + 1);
    }

    fn remove_validator_pending_redelegation(&mut self, slot_idx: usize) {
        let set_size = self.validators_pending_redelegation_set_size();

        if slot_idx >= set_size {
            panic!("invalid slot index: {slot_idx} >= {set_size}");
        }

        let validator_to_remove = self
            .validator_pending_redelegation(slot_idx)
            .expect("validator pending redelegation always set for slot index < set size");

        // We can just clear the set if there is only one validator pending redelegation
        if set_size == 1 {
            self.remove(key::PENDING_REDELEGATE.with(slot_idx).as_bytes());
            self.remove(
                key::PENDING_REDELEGATE_SLOT
                    .with(validator_to_remove)
                    .as_bytes(),
            );
            self.set_validators_pending_redelegation_set_size(0);
            return;
        }

        // If we're not removing the last element, swap with the last one
        if slot_idx < set_size - 1 {
            let last_validator = self
                .validator_pending_redelegation(set_size - 1)
                .expect("validator pending redelegation always set for slot index < set size");

            // Move the last validator to the slot_idx position
            self.set_validator_pending_redelegation(slot_idx, &last_validator);

            // Update the slot mapping for the moved validator
            self.set_validator_pending_redelegation_slot(&last_validator, slot_idx);
        }

        // Remove the last validator entry as it's either been moved or is the one we're removing
        self.remove(key::PENDING_REDELEGATE.with(set_size - 1).as_bytes());

        // Remove the slot mapping for the removed validator
        self.remove(
            key::PENDING_REDELEGATE_SLOT
                .with(validator_to_remove)
                .as_bytes(),
        );

        // Decrease the set size
        self.set_validators_pending_redelegation_set_size(set_size - 1);
    }

    fn validator_pending_redelegation_slot(
        &self,
        validator_pending_redelegation: &str,
    ) -> Option<usize> {
        self.usize_at(key::PENDING_REDELEGATE_SLOT.with(validator_pending_redelegation))
    }

    fn set_validator_pending_redelegation_slot(
        &mut self,
        validator_pending_redelegation: &str,
        slot_idx: usize,
    ) {
        self.set_usize(
            key::PENDING_REDELEGATE_SLOT.with(validator_pending_redelegation),
            slot_idx,
        );
    }

    fn validators_pending_redelegation_set_size(&self) -> usize {
        self.usize_at(key::PENDING_REDELEGATE_SET_SIZE)
            .unwrap_or_default()
    }

    fn set_validators_pending_redelegation_set_size(
        &mut self,
        validators_pending_redelegation_set_size: usize,
    ) {
        self.set_usize(
            key::PENDING_REDELEGATE_SET_SIZE,
            validators_pending_redelegation_set_size,
        )
    }

    fn rewards_sink_address(&self) -> String {
        self.string_at(key::REWARDS_SINK_ADDRESS)
            .expect("set during initialisation")
    }

    fn set_rewards_sink_address(&mut self, rewards_sink_address: &str) {
        self.set_string(key::REWARDS_SINK_ADDRESS, rewards_sink_address);
    }

    fn token_factory_flavour(&self) -> TokenFactoryFlavour {
        self.u8_at(key::TOKEN_FACTORY_FLAVOUR)
            .expect("set during initialisation")
            .into()
    }

    fn set_token_factory_flavour(&mut self, token_factory_flavour: TokenFactoryFlavour) {
        self.set_u8(key::TOKEN_FACTORY_FLAVOUR, token_factory_flavour.into());
    }

    fn unbonding_period(&self) -> u64 {
        self.u64_at(key::UNBONDING_PERIOD)
            .expect("set during initialisation")
    }

    fn set_unbonding_period(&mut self, unbonding_period: u64) {
        self.set_u64(key::UNBONDING_PERIOD, unbonding_period);
    }

    fn validator(&self, slot_idx: usize) -> Option<String> {
        self.string_at(key::VALIDATOR.with(slot_idx))
    }

    fn validators(&self) -> ValidatorSet {
        let set_size = self.validator_set_size();

        let mut validators = ValidatorSet::default();

        for slot_idx in 0..set_size {
            validators.insert(
                self.validator(slot_idx)
                    .expect("validator always set for slot index < set size"),
            );
        }

        validators
    }

    fn set_validator(&mut self, slot_idx: usize, validator: &str) {
        self.set_string(key::VALIDATOR.with(slot_idx), validator)
    }

    fn add_validator(&mut self, validator: &str) {
        let set_size = self.validator_set_size();

        if self.validator_slot(validator).is_some() {
            panic!("validator already exists: {validator}");
        }

        self.set_validator(set_size, validator);
        self.set_validator_slot(validator, set_size);
        self.set_validator_set_size(set_size + 1);
    }

    fn remove_validator(&mut self, slot_idx: usize) {
        let set_size = self.validator_set_size();

        if set_size == 1 {
            panic!("cannot remove the last validator");
        }

        if slot_idx >= set_size {
            panic!("invalid slot index: {slot_idx} >= {set_size}");
        }

        let validator_to_remove = self
            .validator(slot_idx)
            .expect("validator always set for slot index < set size");

        // If we're not removing the last element, swap with the last one
        if slot_idx < set_size - 1 {
            let last_validator = self
                .validator(set_size - 1)
                .expect("validator always set for slot index < set size");

            self.set_validator(slot_idx, &last_validator);

            self.set_validator_slot(&last_validator, slot_idx);
        }

        // Remove the last validator entry as it's either been moved or is the one we're removing
        self.remove(key::VALIDATOR.with(set_size - 1).as_bytes());

        // Remove the slot mapping for the removed validator
        self.remove(key::VALIDATOR_SLOT.with(validator_to_remove).as_bytes());

        self.set_validator_set_size(set_size - 1);
    }

    fn validator_slot(&self, validator: &str) -> Option<usize> {
        self.usize_at(key::VALIDATOR_SLOT.with(validator))
    }

    fn set_validator_slot(&mut self, validator: &str, slot_idx: usize) {
        self.set_usize(key::VALIDATOR_SLOT.with(validator), slot_idx);
    }

    fn validator_set_size(&self) -> usize {
        self.usize_at(key::VALIDATOR_SET_SIZE)
            .expect("set during initialisation")
    }

    fn set_validator_set_size(&mut self, validator_set_size: usize) {
        self.set_usize(key::VALIDATOR_SET_SIZE, validator_set_size)
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}
