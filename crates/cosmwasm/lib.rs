use cosmwasm_schema::cw_serde;

pub mod admin;
pub mod hub;
pub mod mint;
pub mod storage;
pub mod strategy;
pub mod vault;

pub use storage::{MapKey, StorageExt};

#[cw_serde]
pub struct MigrateMsg {}
