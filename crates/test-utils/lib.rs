use std::error::Error;

use ron::ser::{to_string_pretty, PrettyConfig};
use serde::Serialize;

pub trait ToExpectInput {
    fn to_expect_input(&self) -> String;
}

pub fn check(actual: impl ToExpectInput, expected: expect_test::Expect) {
    expected.assert_eq(actual.to_expect_input().as_str());
}

pub fn check_err(actual: impl Error, expected: expect_test::Expect) {
    expected.assert_eq(actual.to_string().as_str());
}

impl<T> ToExpectInput for T
where
    T: Serialize,
{
    fn to_expect_input(&self) -> String {
        to_string_pretty(
            self,
            PrettyConfig::new()
                .compact_arrays(false)
                .indentor("  ".to_owned()),
        )
        .unwrap()
    }
}

/// Within N
#[macro_export]
macro_rules! assert_wn {
    ($n:literal, $left:expr, $right:expr $(,)?) => {
        if !($left >= $right - $n && $left <= $right + $n) {
            panic!("{} is not within 1 of {}", $left, $right);
        }
    };
    ($n:literal, $left:expr, $right:expr, $($arg:tt)+) => {
        if !($left >= $right - $n && $left <= $right + $n) {
            panic!("{} is not within 1 of {}: {}", $left, $right, format_args!($($arg)+));
        }
    };
}

pub mod prelude {
    pub use expect_test::expect;
    pub use rstest::*;

    pub use crate::{assert_wn, check, ToExpectInput};
}
