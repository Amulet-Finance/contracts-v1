use crate::{admin::AdminRole, Decimals, Identifier, Recipient, UnauthorizedError};

pub type Minter = Identifier;
pub type Synthetic = Identifier;
pub type SyntheticAmount = u128;

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Deref)]
#[deref(forward)]
pub struct Ticker(std::rc::Rc<String>);

impl Ticker {
    pub fn new(ticker: impl AsRef<str>) -> Self {
        Self(ticker.as_ref().to_lowercase().into())
    }

    pub fn into_string(self) -> String {
        std::rc::Rc::unwrap_or_clone(self.0)
    }
}

impl From<Ticker> for String {
    fn from(value: Ticker) -> Self {
        value.into_string()
    }
}

impl From<String> for Ticker {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),

    #[error("ticker already exists")]
    TickerAlreadyExists,

    #[error("synthetic not found")]
    SyntheticNotFound,
}

pub enum MintCmd {
    Mint {
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    },
    Burn {
        synthetic: Synthetic,
        amount: SyntheticAmount,
    },
}

pub enum ConfigCmd {
    CreateSynthetic { ticker: Ticker, decimals: Decimals },
    Whitelist { minter: Minter, enabled: bool },
}

pub enum Cmd {
    Config(ConfigCmd),
    Mint(MintCmd),
}
pub trait Repository {
    /// Returns true if the ticker exists
    fn ticker_exists(&self, ticker: &Ticker) -> bool;

    /// Returns true if the synthetic exists
    fn synthetic_exists(&self, synthetic: &Synthetic) -> bool;

    /// Returns Some(true | false) if the minter whitelist status has been set, otherwise None
    fn is_whitelisted(&self, minter: &Minter) -> Option<bool>;
}

pub trait Mint {
    /// Create a synthetic asset ready for minting with the given `ticker` and `decimals` - requires the admin role
    fn create_synthetic(
        &self,
        admin_role: AdminRole,
        ticker: Ticker,
        decimals: Decimals,
    ) -> Result<Cmd, Error>;

    /// Set a minter as whitelisted or not - requires the admin role
    fn set_whitelisted(
        &self,
        admin_role: AdminRole,
        minter: Minter,
        whitelisted: bool,
    ) -> Result<Cmd, Error>;

    /// Mint an amount of synthetics to a recipient
    fn mint(
        &self,
        minter: Minter,
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> Result<Cmd, Error>;

    /// Burn an amount of synthetics, anyone can do this
    fn burn(&self, synthetic: Synthetic, amount: SyntheticAmount) -> Result<Cmd, Error>;
}

pub struct MintImpl<'a>(&'a dyn Repository);

pub fn mint(repository: &dyn Repository) -> MintImpl {
    MintImpl(repository)
}

impl<'a> Mint for MintImpl<'a> {
    fn create_synthetic(
        &self,
        _: AdminRole,
        ticker: Ticker,
        decimals: Decimals,
    ) -> Result<Cmd, Error> {
        if self.0.ticker_exists(&ticker) {
            return Err(Error::TickerAlreadyExists);
        }

        Ok(ConfigCmd::CreateSynthetic { ticker, decimals }.into())
    }

    fn set_whitelisted(
        &self,
        _: AdminRole,
        minter: Minter,
        whitelisted: bool,
    ) -> Result<Cmd, Error> {
        Ok(ConfigCmd::Whitelist {
            minter,
            enabled: whitelisted,
        }
        .into())
    }

    fn mint(
        &self,
        minter: Minter,
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> Result<Cmd, Error> {
        if !self.0.synthetic_exists(&synthetic) {
            return Err(Error::SyntheticNotFound);
        }

        if !self.0.is_whitelisted(&minter).unwrap_or(false) {
            return Err(UnauthorizedError.into());
        }

        Ok(MintCmd::Mint {
            synthetic,
            amount,
            recipient,
        }
        .into())
    }

    fn burn(&self, synthetic: Synthetic, amount: SyntheticAmount) -> Result<Cmd, Error> {
        if !self.0.synthetic_exists(&synthetic) {
            return Err(Error::SyntheticNotFound);
        }

        Ok(MintCmd::Burn { synthetic, amount }.into())
    }
}

impl From<MintCmd> for Cmd {
    fn from(v: MintCmd) -> Self {
        Self::Mint(v)
    }
}

impl From<ConfigCmd> for Cmd {
    fn from(v: ConfigCmd) -> Self {
        Self::Config(v)
    }
}
