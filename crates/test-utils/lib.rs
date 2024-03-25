use ron::ser::{to_string_pretty, PrettyConfig};
use serde::Serialize;

pub trait ToExpectInput {
    fn to_expect_input(&self) -> String;
}

pub fn check(actual: impl ToExpectInput, expected: expect_test::Expect) {
    expected.assert_eq(actual.to_expect_input().as_str());
}

impl<T> ToExpectInput for T
where
    T: Serialize,
{
    fn to_expect_input(&self) -> String {
        to_string_pretty(
            self,
            PrettyConfig::new()
                .compact_arrays(true)
                .indentor("  ".to_owned()),
        )
        .unwrap()
    }
}

pub mod prelude {
    pub use expect_test::expect;
    pub use rstest::*;

    pub use crate::{check, ToExpectInput};
}
