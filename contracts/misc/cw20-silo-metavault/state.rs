use amulet_cw::StorageExt as _;
use cosmwasm_std::Storage;

#[rustfmt::skip]
mod key {
    macro_rules! key {
        ($k:literal) => {
            concat!("cw20_silo::", $k)
        };
    }

    pub const OWNER               : &str = key!("owner");
    pub const CW20                : &str = key!("cw20");
    pub const HUB                 : &str = key!("hub");
    pub const UNDERLYING_DECIMALS : &str = key!("underlying_decimals");
    pub const DEPOSITS            : &str = key!("deposits");
}

pub trait StorageExt: Storage {
    fn owner(&self) -> String {
        self.string_at(key::OWNER)
            .expect("set during initialisation")
    }

    fn cw20(&self) -> String {
        self.string_at(key::CW20)
            .expect("set during initialisation")
    }

    fn hub(&self) -> String {
        self.string_at(key::HUB).expect("set during initialisation")
    }

    fn underlying_decimals(&self) -> u32 {
        self.u32_at(key::UNDERLYING_DECIMALS)
            .expect("set during initialisation")
    }

    fn deposits(&self) -> u128 {
        self.u128_at(key::DEPOSITS).unwrap_or_default()
    }

    fn set_owner(&mut self, owner: &str) {
        self.set_string(key::OWNER, owner);
    }

    fn set_cw20(&mut self, cw20: &str) {
        self.set_string(key::CW20, cw20);
    }

    fn set_hub(&mut self, hub: &str) {
        self.set_string(key::HUB, hub);
    }

    fn set_underlying_decimals(&mut self, decimals: u32) {
        self.set_u32(key::UNDERLYING_DECIMALS, decimals);
    }

    fn set_deposits(&mut self, deposits: u128) {
        self.set_u128(key::DEPOSITS, deposits);
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}
