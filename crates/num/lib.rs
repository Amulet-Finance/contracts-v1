#[allow(clippy::assign_op_pattern)]
mod uint {
    uint::construct_uint! {
        pub struct U256(4);
    }

    uint::construct_uint! {
        pub struct U512(8);
    }
}

use std::fmt::Write;

pub use uint::{U256, U512};

impl U256 {
    pub fn from_be_bytes(be_bytes: [u8; 8 * 4]) -> Self {
        Self::from_big_endian(&be_bytes)
    }

    pub fn to_be_bytes(self) -> [u8; 8 * 4] {
        let mut u256_be_bytes = [0u8; 8 * 4];

        self.to_big_endian(&mut u256_be_bytes);

        u256_be_bytes
    }

    /// Returns `Some((self * m) / d)` if d > 0, otherwise `None`
    pub fn checked_mul_div(self, m: u128, d: u128) -> Option<Self> {
        if d == 0 {
            return None;
        }

        Some((self * Self::from(m)) / d)
    }
}

impl From<U256> for U512 {
    fn from(value: U256) -> Self {
        let mut u256_le_bytes = [0u8; 8 * 4];

        value.to_little_endian(&mut u256_le_bytes);

        Self::from_little_endian(&u256_le_bytes)
    }
}

impl serde::Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_newtype_struct("U256", self.to_string().as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OverflowError;

impl TryFrom<U512> for U256 {
    type Error = OverflowError;

    fn try_from(value: U512) -> Result<Self, Self::Error> {
        if value.bits() > 256 {
            return Err(OverflowError);
        }

        let mut u512_le_bytes = [0u8; 8 * 8];

        value.to_little_endian(&mut u512_le_bytes);

        Ok(Self::from_little_endian(&u512_le_bytes[..(8 * 4)]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedU256(U256);

impl FixedU256 {
    pub const FRAC_BITS: u32 = 128;

    pub const fn raw(x: U256) -> Self {
        Self(x)
    }

    pub const fn zero() -> Self {
        Self(U256::zero())
    }

    pub fn from_u128(x: u128) -> Self {
        Self(U256::from(x) << 128)
    }

    pub const fn into_raw(self) -> U256 {
        self.0
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        if self.0.is_zero() || rhs.0.is_zero() {
            return Some(Self(U256::zero()));
        }

        let lhs = U512::from(self.0);

        let rhs = U512::from(rhs.0);

        let ans = lhs.checked_mul(rhs)? >> Self::FRAC_BITS;

        ans.try_into().ok().map(Self)
    }

    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0.is_zero() {
            return None;
        }

        if self.0.is_zero() {
            return Some(Self(U256::zero()));
        }

        let lhs = U512::from(self.0);

        let rhs = U512::from(rhs.0);

        let ans = (lhs << Self::FRAC_BITS).checked_div(rhs)?;

        ans.try_into().ok().map(Self)
    }

    pub fn floor(self) -> u128 {
        (self.0 >> Self::FRAC_BITS)
            .try_into()
            .expect("safe conversion to u128")
    }

    pub fn abs_diff(self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }

    pub const fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl serde::Serialize for FixedU256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_newtype_struct("FixedU256", self.to_string().as_str())
    }
}

impl std::fmt::Display for FixedU256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let integer = self.floor();

        let FixedU256(U256([f0, f1, ..])) = self;

        let mut fractional = U256([*f0, *f1, 0, 0]);

        write!(f, "{integer}.")?;

        let mut digit_count = 0;

        let mut fractional_str = String::with_capacity(32);

        while fractional > U256::zero() && digit_count < 32 {
            let U256([f0, f1, i0, ..]) = fractional * 10;

            let digit = u32::try_from(i0).unwrap();

            fractional_str.push(char::from_digit(digit, 10).unwrap());

            digit_count += 1;

            fractional = U256([f0, f1, 0, 0]);
        }

        let fractional_str = fractional_str.trim_end_matches('0');

        if fractional_str.is_empty() {
            f.write_char('0')
        } else {
            f.write_str(fractional_str)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn from_ratio(numer: u128, denom: u128) -> FixedU256 {
        FixedU256::from_u128(numer)
            .checked_div(FixedU256::from_u128(denom))
            .unwrap()
    }

    #[test]
    fn u256_checked_mul_div() {
        assert_eq!(U256::one().checked_mul_div(10, 0), None);

        assert_eq!(
            U256::from(180_000_000_000_000_000_000_000u128)
                .checked_mul_div(270_000_000_000, 270_000_000_000_000_000_000_000),
            Some(U256::from(180_000_000_000u128))
        );
    }

    #[test]
    fn u512_from_u256() {
        assert_eq!(U512::from(U256::zero()), U512::zero());
        assert_eq!(U512::from(U256::one()), U512::one());
        assert_eq!(
            U512::from(U256::max_value()).to_string(),
            U256::max_value().to_string()
        );
    }

    #[test]
    fn u256_try_from_u512() {
        assert_eq!(U256::try_from(U512::zero()).unwrap(), U256::zero());
        assert_eq!(U256::try_from(U512::one()).unwrap(), U256::one());
        assert_eq!(
            U256::try_from(U512::from(U256::max_value())).unwrap(),
            U256::max_value()
        );
        assert!(U256::try_from(U512::from(U256::max_value()) + U512::one()).is_err());
        assert!(U256::try_from(U512::max_value()).is_err());
    }

    #[test]
    fn fixed256_checked_add() {
        let zero = FixedU256::from_u128(0);
        let one = FixedU256::from_u128(1);
        let u128_max = FixedU256::from_u128(u128::MAX);
        let half = from_ratio(1, 2);
        let quarter = from_ratio(1, 4);
        let three_quarters = from_ratio(3, 4);

        assert_eq!(half.checked_add(half).unwrap(), one);
        assert_eq!(three_quarters.checked_add(quarter).unwrap(), one);
        assert_eq!(half.checked_add(quarter).unwrap(), three_quarters);
        assert_eq!(zero.checked_add(zero).unwrap(), zero);
        assert_eq!(zero.checked_add(one).unwrap(), one);

        assert!(u128_max.checked_add(u128_max).is_none());
    }

    #[test]
    fn fixed256_checked_sub() {
        let zero = FixedU256::from_u128(0);
        let one = FixedU256::from_u128(1);
        let half = from_ratio(1, 2);
        let quarter = from_ratio(1, 4);
        let three_quarters = from_ratio(3, 4);

        assert_eq!(half.checked_sub(half).unwrap(), zero);
        assert_eq!(one.checked_sub(half).unwrap(), half);
        assert_eq!(three_quarters.checked_sub(half).unwrap(), quarter);
        assert_eq!(half.checked_sub(quarter).unwrap(), quarter);

        assert!(half.checked_sub(three_quarters).is_none());
    }

    #[test]
    fn fixed256_checked_mul() {
        let zero = FixedU256::from_u128(0);
        let one = FixedU256::from_u128(1);
        let u128_max = FixedU256::from_u128(u128::MAX);
        let half = from_ratio(1, 2);
        let quarter = from_ratio(1, 4);

        assert_eq!(half.checked_mul(half).unwrap(), quarter);
        assert_eq!(one.checked_mul(half).unwrap(), half);
        assert_eq!(one.checked_mul(zero).unwrap(), zero);

        assert!(u128_max.checked_mul(u128_max).is_none());
    }

    #[test]
    fn fixed256_checked_div() {
        let zero = FixedU256::from_u128(0);
        let one = FixedU256::from_u128(1);
        let two = FixedU256::from_u128(2);
        let u128_max = FixedU256::from_u128(u128::MAX);
        let half = from_ratio(1, 2);

        assert_eq!(one.checked_div(half).unwrap(), two);
        assert_eq!(one.checked_div(two).unwrap(), half);
        assert_eq!(u128_max.checked_div(one).unwrap(), u128_max);

        assert!(u128_max.checked_div(zero).is_none());
        assert!(zero.checked_div(zero).is_none());
    }

    #[test]
    fn fixed256_display() {
        let numer = FixedU256::from_u128(1_500);
        let denom = FixedU256::from_u128(1_000);

        let x = numer.checked_div(denom).unwrap();

        assert_eq!(x.to_string(), "1.5");

        let numer = FixedU256::from_u128(1);
        let denom = FixedU256::from_u128(3);

        let x = numer.checked_div(denom).unwrap();

        assert_eq!(x.to_string(), "0.33333333333333333333333333333333");

        let numer = FixedU256::from_u128(1);
        let denom = FixedU256::from_u128(1);

        let x = numer.checked_div(denom).unwrap();

        assert_eq!(x.to_string(), "1.0");
    }
}
