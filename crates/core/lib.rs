pub mod admin;
pub mod hub;
pub mod mint;
pub mod num;
pub mod vault;

use num::FixedU256;

#[derive(
    Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Deref, derive_more::From,
)]
#[deref(forward)]
#[from(forward)]
pub struct Identifier(std::rc::Rc<String>);

pub type Asset = Identifier;
pub type Recipient = Identifier;
pub type Sender = Identifier;
pub type Decimals = u32;

impl Identifier {
    pub fn into_string(self) -> String {
        std::rc::Rc::unwrap_or_clone(self.0)
    }
}

impl From<Identifier> for String {
    fn from(value: Identifier) -> Self {
        value.into_string()
    }
}

impl From<&Identifier> for String {
    fn from(value: &Identifier) -> Self {
        value.clone().into_string()
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unauthorized")]
pub struct UnauthorizedError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Rate(FixedU256);

impl Rate {
    fn one() -> Self {
        Self(FixedU256::from_u128(1))
    }

    fn from_ratio(numer: u128, denom: u128) -> Option<Self> {
        FixedU256::from_u128(numer)
            .checked_div(FixedU256::from_u128(denom))
            .map(Self)
    }

    fn apply_u128(self, x: u128) -> Option<u128> {
        self.0
            .checked_mul(FixedU256::from_u128(x))
            .map(FixedU256::floor)
    }
}

// convenience macro to create a Vec<Cmd> from different command types
macro_rules! cmds {
    ($($cmd:expr),+) => { vec![$($cmd.into()),*] };
}

pub(crate) use cmds;
