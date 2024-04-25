#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum Ica {
    Main = 0,
    Rewards = 1,
}

impl Ica {
    const MAIN_ID: &'static str = "main";
    const REWARDS_ID: &'static str = "rewards";

    pub const fn id(&self) -> &str {
        match self {
            Ica::Main => Self::MAIN_ID,
            Ica::Rewards => Self::REWARDS_ID,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            Self::MAIN_ID => Some(Self::Main),
            Self::REWARDS_ID => Some(Self::Rewards),
            _ => None,
        }
    }
}

impl From<u8> for Ica {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Main,
            1 => Self::Rewards,
            _ => panic!("unexpected ica: {value}"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum Icq {
    MainBalance = 0,
    RewardsBalance = 1,
    MainDelegations = 2,
}

impl Icq {
    pub const MAIN_BALANCE_ID: &'static str = "main_balance";
    pub const REWARDS_BALANCE_ID: &'static str = "rewards_balance";
    pub const MAIN_DELEGATIONS_ID: &'static str = "main_delegations";

    pub const fn id(&self) -> &str {
        match self {
            Self::MainBalance => Self::MAIN_BALANCE_ID,
            Self::RewardsBalance => Self::REWARDS_BALANCE_ID,
            Self::MainDelegations => Self::MAIN_DELEGATIONS_ID,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            Self::MAIN_BALANCE_ID => Some(Self::MainBalance),
            Self::REWARDS_BALANCE_ID => Some(Self::RewardsBalance),
            Self::MAIN_DELEGATIONS_ID => Some(Self::MainDelegations),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TotalActualUnbonded(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TotalExpectedUnbonded(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AvailableToClaim(pub u128);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbondingAckCount(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbondingIssuedCount(pub u64);
