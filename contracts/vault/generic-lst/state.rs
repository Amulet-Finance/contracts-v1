use amulet_cw::StorageExt as _;
use cosmwasm_std::Storage;

#[rustfmt::skip]
mod key {
    macro_rules! key {
        ($k:literal) => {
            concat!("generic_lst::", $k)
        };
    }

    pub const LST_REDEMPTION_RATE_ORACLE : &str = key!("lst_redemption_rate_oracle");
    pub const LST_DENOM                  : &str = key!("lst_denom");
    pub const LST_DECIMALS               : &str = key!("lst_decimals");
    pub const UNDERLYING_DECIMALS        : &str = key!("underlying_decimals");
    pub const ACTIVE_LST_BALANCE         : &str = key!("active_lst_balance");
    pub const CLAIMABLE_LST_BALANCE      : &str = key!("claimable_lst_balance");
}

pub trait StorageExt: Storage {
    fn lst_redemption_rate_oracle(&self) -> String {
        self.string_at(key::LST_REDEMPTION_RATE_ORACLE)
            .expect("set during initialisation")
    }

    fn lst_denom(&self) -> String {
        self.string_at(key::LST_DENOM)
            .expect("set during initialisation")
    }

    fn lst_decimals(&self) -> u32 {
        self.u32_at(key::LST_DECIMALS)
            .expect("set during initialisation")
    }

    fn underlying_decimals(&self) -> u32 {
        self.u32_at(key::UNDERLYING_DECIMALS)
            .expect("set during initialisation")
    }

    fn active_lst_balance(&self) -> u128 {
        self.u128_at(key::ACTIVE_LST_BALANCE).unwrap_or_default()
    }

    fn claimable_lst_balance(&self) -> u128 {
        self.u128_at(key::CLAIMABLE_LST_BALANCE).unwrap_or_default()
    }

    fn set_lst_redemption_rate_oracle(&mut self, oracle: &str) {
        self.set_string(key::LST_REDEMPTION_RATE_ORACLE, oracle);
    }

    fn set_lst_denom(&mut self, denom: &str) {
        self.set_string(key::LST_DENOM, denom);
    }

    fn set_lst_decimals(&mut self, decimals: u32) {
        self.set_u32(key::LST_DECIMALS, decimals);
    }

    fn set_underlying_decimals(&mut self, decimals: u32) {
        self.set_u32(key::UNDERLYING_DECIMALS, decimals);
    }

    fn set_active_lst_balance(&mut self, balance: u128) {
        self.set_u128(key::ACTIVE_LST_BALANCE, balance)
    }

    fn set_claimable_lst_balance(&mut self, balance: u128) {
        self.set_u128(key::CLAIMABLE_LST_BALANCE, balance)
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}
