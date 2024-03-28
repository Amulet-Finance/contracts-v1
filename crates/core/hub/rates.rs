macro_rules! bps_rate {
    ($T:ident, max=$max:expr, default=$default:expr) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub struct $T {
            bps: u32,
            rate: $crate::Rate,
        }

        impl $T {
            pub const MAX: u32 = $max;

            pub fn new(bps: u32) -> Option<Self> {
                if bps > Self::MAX {
                    return None;
                }

                let rate = $crate::Rate::from_ratio(bps.into(), 10_000).unwrap();

                Some(Self { bps, rate })
            }

            pub fn raw(self) -> u32 {
                self.bps
            }

            pub(crate) fn rate(self) -> $crate::Rate {
                self.rate
            }
        }

        ::static_assertions::const_assert!($default <= $max);

        impl Default for $T {
            fn default() -> Self {
                Self::new($default).unwrap()
            }
        }
    };
}

macro_rules! percent {
    ($x:literal) => {
        $x * 100
    };
}

bps_rate!(MaxLtv, max = percent!(100), default = percent!(50));

bps_rate!(
    CollateralYieldFee,
    max = percent!(100),
    default = percent!(10)
);

bps_rate!(
    ReserveYieldFee,
    max = percent!(100),
    default = percent!(100)
);

bps_rate!(
    AdvanceFee,
    max = percent!(50),
    default = 25 // bps: 0.25%
);

bps_rate!(
    AmoAllocation,
    max = percent!(100),
    default = 0 // bps: 0.0%
);
