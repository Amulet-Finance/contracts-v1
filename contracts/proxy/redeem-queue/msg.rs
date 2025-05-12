use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{coins, to_json_binary, Uint128, WasmMsg};

use amulet_cw::{
    admin::{ExecuteMsg as AdminExecuteMsg, QueryMsg as AdminQueryMsg},
    hub::UserMsg as HubMsg,
};

#[cw_serde]
pub struct InstantiateMsg {
    pub hub_address: String,
}

#[cw_serde]
pub enum ProxyUserMsg {
    /// Attempts to process the redemption queue for the specified vault.
    /// Redeems pending amounts from the head of the queue while there is
    /// sufficient redeemable balance or until the queue is empty.
    ProcessHead { vault: String },

    /// Enters the redemption queue for a given vault by sending one non-zero token,
    /// which must be the vault's associated synthetic token. If the final queue entry
    /// (tail) belongs to the same sender, the sent amount is appended to that entry;
    /// otherwise, a new tail entry is created.
    ///
    /// If the queue is empty, redemption is attempted immediately using any available
    /// redeemable balance, with any leftover amount forming the new head entry. If the
    /// queue is not empty, the existing queue is processed first (similar to `ProcessHead`)
    /// before adding the new entry.
    Redeem { vault: String },

    /// Cancels a specific queue entry in the specified vault’s redemption queue, which
    /// must be owned by the sender. The associated balance is returned to the sender.
    CancelEntry { index: u64, vault: String },

    /// Cancels all redemption queue entries for the specified vault that belong to the
    /// sender. The total associated balance is returned to the sender.
    CancelAll { vault: String },
}

#[cw_serde]
pub enum ProxyAdminMsg {
    /// Forcefully cancels a specific queue entry for the given vault, returning the
    /// associated balance to the entry's owner.
    ForceCancelEntry { vault: String, index: u64 },
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminExecuteMsg),
    ProxyUser(ProxyUserMsg),
    ProxyAdmin(ProxyAdminMsg),
}

#[cw_serde]
pub struct ConfigResponse {
    pub hub_address: String,
}

#[cw_serde]
pub struct QueueEntry {
    pub index: u64,
    pub address: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct QueueEntriesResponse {
    pub entries: Vec<QueueEntry>,
}

#[cw_serde]
pub struct QueueEntryResponse {
    pub entry: QueueEntry,
    pub position_in_queue: u64,
    pub amount_in_front: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ProxyQueryMsg {
    /// Returns the current configuration of the contract, including the hub address.
    #[returns(ConfigResponse)]
    Config {},

    /// Retrieves all redemption queue entries for the specified vault.
    /// Supports optional pagination via `start_index` and `limit`.
    /// Returns a `QueueEntriesResponse` containing the requested entries.
    #[returns(QueueEntriesResponse)]
    AllQueueEntries {
        vault: String,
        start_index: Option<u64>,
        limit: Option<u64>,
    },

    /// Retrieves the redemption queue entries owned by a particular address for
    /// the specified vault. Supports optional pagination via `start` and `limit`.
    /// Returns a `QueueEntriesResponse` containing the matching entries.
    #[returns(QueueEntriesResponse)]
    OwnerQueueEntries {
        vault: String,
        address: String,
        start: Option<u64>,
        limit: Option<u64>,
    },

    /// Provides detailed information about a specific queue entry in the specified
    /// vault, identified by its `index`. Returns a `QueueEntryResponse` that includes
    /// the entry's position in the queue and the total amount of pending redemptions
    /// "in front" of it.
    #[returns(QueueEntryResponse)]
    QueueEntry { vault: String, index: u64 },
}

#[cw_serde]
#[derive(QueryResponses)]
#[serde(untagged)]
#[query_responses(nested)]
pub enum QueryMsg {
    Admin(AdminQueryMsg),
    Proxy(ProxyQueryMsg),
}

pub fn redeem_on_behalf(
    hub: &str,
    vault: &str,
    synthetic: &str,
    behalf_of: String,
    amount: u128,
) -> WasmMsg {
    WasmMsg::Execute {
        contract_addr: hub.to_owned(),
        msg: to_json_binary(&HubMsg::RedeemOnBehalf {
            vault: vault.to_owned(),
            behalf_of,
        })
        .expect("infallible serialization"),
        funds: coins(amount, synthetic),
    }
}
