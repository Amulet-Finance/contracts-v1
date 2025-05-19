use amulet_token_factory::Flavour as TokenFactoryFlavour;
use cosmwasm_schema::{cw_serde, QueryResponses};

use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    vault::{ExecuteMsg as VaultExecuteMsg, QueryMsg as VaultQueryMsg},
};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub rewards_sink_code_id: u64,
    pub rewards_sink_code_hash: String,
    pub token_factory_flavour: TokenFactoryFlavour,
    pub bond_denom: String,
    pub bond_denom_decimals: u32,
    pub max_unbonding_entries: u64,
    pub unbonding_period: u64,
    pub initial_validator_set: Vec<String>,
}

#[cw_serde]
pub enum StrategyExecuteMsg {
    ///  Collect staking rewards and delegate them across the validator set
    ///
    /// # Requirements
    /// * Anyone can issue this message
    ///
    /// # Errors
    /// * Fails if there are no pending rewards
    CollectRewards {},

    ///  Acknowledge any completed unbonding batches ready for claiming, altering received amounts if slashed.
    ///
    /// # Requirements
    /// * Anyone can issue this message
    AcknowledgeUnbondings {},

    ///  Process pending redelegations by starting any redelegation for a validator that has a non-zero elligible balance.
    ///  If the elligible balance equals the total delegated amount, remove the validator from pending redelegation set.
    ///
    /// # Requirements
    /// * Anyone can issue this message
    ///
    /// # Errors
    /// * Fails if there are no pending redelegations within the `start` and `limit` range
    /// * Fails if there none of the pending redelegations are elligible to be started
    ProcessRedelegations {
        start: Option<usize>,
        limit: Option<usize>,
    },

    /// Remove a validator from the validator set, redelegating staked assets across the remaining validators.
    ///
    /// # Requirements
    /// * Admin role
    ///
    /// # Errors
    /// * Fails if the validator does not exist in the vault set
    /// * Fails if there is only one validator in the vault set
    RemoveValidator { validator: String },

    /// Add a validator to the set to receive delegations.
    ///
    /// # Requirements
    /// * Admin role
    ///
    /// # Errors
    /// * Fails if the validator is does not exist in the active global set
    /// * Fails if the validator is currently pending redelegation
    /// * Fails if the validator is already in the vault set
    AddValidator { validator: String },

    /// Remove the from_validator from the set and add the to_validator,
    /// redelegating entire stake from the removed validator to the added validator.
    ///
    /// # Requirements
    /// * Admin role
    ///
    /// # Errors
    /// * Fails if the from_validator is not in the set
    /// * Fails if the to_validator does not exist
    /// * Fails if the to_validator is already in the vault active set or pending redelegation set
    SwapValidator {
        from_validator: String,
        to_validator: String,
    },
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    Vault(VaultExecuteMsg),
    Strategy(StrategyExecuteMsg),
}

#[cw_serde]
pub struct Config {
    pub bond_denom: String,
    pub bond_denom_decimals: u32,
    pub max_unbonding_entries: u64,
    pub unbonding_period: u64,
}

#[cw_serde]
pub struct Metadata {
    pub available_to_claim: Uint128,
    pub delegated: Uint128,
    pub last_unbond_timestamp: u64,
    pub last_acknowledged_batch: Option<u64>,
    pub minimum_unbond_interval: u64,
    pub rewards_sink_address: String,
}

#[cw_serde]
pub struct ValidatorSet {
    pub size: usize,
    pub active_validators: Vec<String>,
    pub validators_pending_redelegation: Vec<String>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum StrategyQueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(Metadata)]
    Metadata {},
    #[returns(ValidatorSet)]
    ValidatorSet {},
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Vault(VaultQueryMsg),
    Strategy(StrategyQueryMsg),
}
