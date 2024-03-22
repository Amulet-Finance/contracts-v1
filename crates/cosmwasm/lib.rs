use amulet_core::num::U256;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, MessageInfo, Storage};

pub mod admin;
pub mod hub;
pub mod mint;
pub mod strategy;
pub mod vault;

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("non payable - received unexpected funds")]
    NonPayable,
    #[error("payable - did not receive any funds")]
    Payable,
    #[error("more than one asset sent")]
    MoreThanOneAssetSent,
}

pub fn non_payable(info: &MessageInfo) -> Result<(), PaymentError> {
    if info.funds.is_empty() {
        return Ok(());
    };

    Err(PaymentError::NonPayable)
}

pub fn one_coin(info: &MessageInfo) -> Result<Coin, PaymentError> {
    match info.funds.as_slice() {
        [] => Err(PaymentError::Payable),

        [coin] => Ok(coin.clone()),

        _ => Err(PaymentError::MoreThanOneAssetSent),
    }
}

pub trait StorageExt: Storage {
    /// Returns true if there is something stored at the `key`
    fn has_key(&self, key: impl AsRef<str>) -> bool {
        self.get(key.as_ref().as_bytes()).is_some()
    }

    /// Fetch data stored at `key` (if any) as a UTF-8 string
    /// Panics if the stored bytes are invalid UTF-8
    fn string_at(&self, key: impl AsRef<str>) -> Option<String> {
        self.get(key.as_ref().as_bytes())
            .map(String::from_utf8)
            .transpose()
            .expect("valid utf-8 bytes if present")
    }

    /// Fetch data stored at `key` (if any) as a U256
    /// Panics if the stored bytes do not exactly constitute a U256
    fn u256_at(&self, key: impl AsRef<str>) -> Option<U256> {
        self.get(key.as_ref().as_bytes())
            .map(TryFrom::try_from)
            .transpose()
            .expect("exactly 32 bytes if present")
            .map(U256::from_be_bytes)
    }

    /// Fetch data stored at `key` (if any) as a u128
    /// Panics if the stored bytes do not exactly constitute a u128
    fn u128_at(&self, key: impl AsRef<str>) -> Option<u128> {
        self.get(key.as_ref().as_bytes())
            .map(TryFrom::try_from)
            .transpose()
            .expect("exactly 16 bytes if present")
            .map(u128::from_be_bytes)
    }

    /// Fetch data stored at `key` (if any) as a u64
    /// Panics if the stored bytes do not exactly constitute a u64
    fn u64_at(&self, key: impl AsRef<str>) -> Option<u64> {
        self.get(key.as_ref().as_bytes())
            .map(TryFrom::try_from)
            .transpose()
            .expect("exactly 8 bytes if present")
            .map(u64::from_be_bytes)
    }

    /// Fetch data stored at `key` (if any) as a u32
    /// Panics if the stored bytes do not exactly constitute a u32
    fn u32_at(&self, key: impl AsRef<str>) -> Option<u32> {
        self.get(key.as_ref().as_bytes())
            .map(TryFrom::try_from)
            .transpose()
            .expect("exactly 4 bytes if present")
            .map(u32::from_be_bytes)
    }

    /// Fetch data stored at `key` (if any) as a boolean
    /// Panics if there is not only one byte stored, if any
    fn bool_at(&self, key: impl AsRef<str>) -> Option<bool> {
        self.get(key.as_ref().as_bytes())
            .map(TryFrom::try_from)
            .transpose()
            .expect("exactly 1 byte if present")
            .map(|[b]: [u8; 1]| b == 1)
    }

    /// Set the data stored at `key` to the UTF-8 string `s`
    fn set_string(&mut self, key: impl AsRef<str>, s: &str) {
        self.set(key.as_ref().as_bytes(), s.as_bytes());
    }

    /// Set the data stored at `key` to the U256 `x`
    fn set_u256(&mut self, key: impl AsRef<str>, x: U256) {
        self.set(key.as_ref().as_bytes(), &x.to_be_bytes())
    }

    /// Set the data stored at `key` to the u128 `x`
    fn set_u128(&mut self, key: impl AsRef<str>, x: u128) {
        self.set(key.as_ref().as_bytes(), &x.to_be_bytes())
    }

    /// Set the data stored at `key` to the u64 `x`
    fn set_u64(&mut self, key: impl AsRef<str>, x: u64) {
        self.set(key.as_ref().as_bytes(), &x.to_be_bytes())
    }

    /// Set the data stored at `key` to the u32 `x`
    fn set_u32(&mut self, key: impl AsRef<str>, x: u32) {
        self.set(key.as_ref().as_bytes(), &x.to_be_bytes())
    }

    /// Set the data stored at `key` to the u32 `x`
    fn set_bool(&mut self, key: impl AsRef<str>, b: bool) {
        self.set(key.as_ref().as_bytes(), &[b as u8])
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

pub struct MapKey(&'static str);

impl MapKey {
    pub const fn new(prefix: &'static str) -> Self {
        Self(prefix)
    }

    pub fn with(self, t: impl ToString) -> String {
        self.multi([&t])
    }

    pub fn multi<const N: usize>(self, ts: [&dyn ToString; N]) -> String {
        let mut s = String::with_capacity(1024);

        s.push_str(self.0);

        for t in ts {
            s.push_str(&t.to_string());
            s.push(':');
        }

        s
    }
}

#[cw_serde]
pub struct MigrateMsg {}
