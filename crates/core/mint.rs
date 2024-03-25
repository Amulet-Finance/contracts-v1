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

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),

    #[error("ticker already exists")]
    TickerAlreadyExists,

    #[error("synthetic not found")]
    SyntheticNotFound,
}

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigCmd {
    CreateSynthetic { ticker: Ticker, decimals: Decimals },
    Whitelist { minter: Minter, enabled: bool },
}

#[derive(Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use test_utils::prelude::*;

    use super::{mint as make_mint, *};

    #[derive(Default)]
    struct Context {
        tickers: HashSet<String>,
        synthetic: HashSet<String>,
        whitelist: HashSet<String>,
    }

    impl Context {
        fn handle_cmd(&mut self, cmd: Cmd) {
            match cmd {
                Cmd::Config(config_cmd) => match config_cmd {
                    ConfigCmd::CreateSynthetic { ticker, .. } => {
                        if self.tickers.contains(ticker.as_str())
                            || self.synthetic.contains(ticker.as_str())
                        {
                            panic!("synthetic already exisits")
                        }

                        self.tickers.insert(ticker.clone().into_string());
                        self.synthetic.insert(ticker.into_string());
                    }

                    ConfigCmd::Whitelist { minter, enabled } => {
                        if enabled {
                            self.whitelist.insert(minter.into_string());
                        } else {
                            self.whitelist.remove(minter.as_str());
                        }
                    }
                },
                Cmd::Mint(mint_cmd) => match mint_cmd {
                    MintCmd::Mint { synthetic, .. } | MintCmd::Burn { synthetic, .. } => {
                        assert!(self.synthetic.contains(synthetic.as_str()))
                    }
                },
            }
        }

        fn after_cmd(mut self, cmd: Cmd) -> Self {
            self.handle_cmd(cmd);
            self
        }
    }

    impl Repository for Context {
        fn ticker_exists(&self, ticker: &Ticker) -> bool {
            self.tickers.contains(ticker.as_str())
        }

        fn synthetic_exists(&self, synthetic: &Synthetic) -> bool {
            self.synthetic.contains(synthetic.as_str())
        }

        fn is_whitelisted(&self, minter: &Minter) -> Option<bool> {
            self.whitelist.contains(minter.as_str()).then_some(true)
        }
    }

    fn am_asset_ticker() -> Ticker {
        "amASSET".to_owned().into()
    }

    fn am_asset() -> Synthetic {
        "amasset".to_owned().into()
    }

    fn phantom_asset_ticker() -> Ticker {
        "booASSET".to_owned().into()
    }

    fn phantom_asset() -> Synthetic {
        "booasset".to_owned().into()
    }

    fn whitelisted_minter() -> Minter {
        "minter".to_owned().into()
    }

    fn non_whitelisted_minter() -> Minter {
        "non_minter".to_owned().into()
    }

    fn mint_recipient() -> Recipient {
        "recipient".to_owned().into()
    }

    impl Cmd {
        fn create_synthetic(ticker: Ticker, decimals: Decimals) -> Self {
            ConfigCmd::CreateSynthetic { ticker, decimals }.into()
        }

        fn set_whitelisted(minter: Minter, enabled: bool) -> Self {
            ConfigCmd::Whitelist { minter, enabled }.into()
        }

        fn mint(synthetic: Synthetic, amount: SyntheticAmount, recipient: Recipient) -> Self {
            MintCmd::Mint {
                synthetic,
                amount,
                recipient,
            }
            .into()
        }

        fn burn(synthetic: Synthetic, amount: SyntheticAmount) -> Self {
            MintCmd::Burn { synthetic, amount }.into()
        }
    }

    #[fixture]
    fn admin_role() -> AdminRole {
        AdminRole::mock()
    }

    #[fixture]
    fn ctx() -> Context {
        Context::default()
            .after_cmd(Cmd::create_synthetic(am_asset_ticker(), 6))
            .after_cmd(Cmd::set_whitelisted(whitelisted_minter(), true))
    }

    #[rstest]
    #[case::ticker_available(
        phantom_asset_ticker(),
        6,
        Ok(Cmd::create_synthetic(phantom_asset_ticker(), 6))
    )]
    #[case::ticker_taken(am_asset_ticker(), 6, Err(Error::TickerAlreadyExists))]
    fn create_synthetic(
        admin_role: AdminRole,
        mut ctx: Context,
        #[case] ticker: Ticker,
        #[case] decimals: Decimals,
        #[case] expected: Result<Cmd, Error>,
    ) {
        let actual = make_mint(&ctx).create_synthetic(admin_role, ticker, decimals);

        assert_eq!(actual, expected);

        if let Ok(cmd) = actual {
            ctx.handle_cmd(cmd)
        }
    }

    #[rstest]
    #[case::set_true(
        whitelisted_minter(),
        true,
        Ok(Cmd::set_whitelisted(whitelisted_minter(), true))
    )]
    #[case::set_false(
        non_whitelisted_minter(),
        false,
        Ok(Cmd::set_whitelisted(non_whitelisted_minter(), false))
    )]
    fn set_whitelisted(
        admin_role: AdminRole,
        mut ctx: Context,
        #[case] minter: Minter,
        #[case] enabled: bool,
        #[case] expected: Result<Cmd, Error>,
    ) {
        let actual = make_mint(&ctx).set_whitelisted(admin_role, minter, enabled);

        assert_eq!(actual, expected);

        if let Ok(cmd) = actual {
            ctx.handle_cmd(cmd)
        }
    }

    #[rstest]
    #[case::whitelisted_minter_existing_synthetic(
        whitelisted_minter(),
        am_asset(),
        1_000_000,
        mint_recipient(),
        Ok(Cmd::mint(am_asset(), 1_000_000, mint_recipient()))
    )]
    #[case::whitelisted_minter_non_existing_synthetic(
        whitelisted_minter(),
        phantom_asset(),
        1_000_000,
        mint_recipient(),
        Err(Error::SyntheticNotFound)
    )]
    #[case::non_whitelisted_minter_existing_synthetic(
        non_whitelisted_minter(),
        am_asset(),
        1_000_000,
        mint_recipient(),
        Err(UnauthorizedError.into())
    )]
    fn mint(
        mut ctx: Context,
        #[case] minter: Minter,
        #[case] synthetic: Synthetic,
        #[case] amount: SyntheticAmount,
        #[case] recipient: Recipient,
        #[case] expected: Result<Cmd, Error>,
    ) {
        let actual = make_mint(&ctx).mint(minter, synthetic, amount, recipient);

        assert_eq!(actual, expected);

        if let Ok(cmd) = actual {
            ctx.handle_cmd(cmd)
        }
    }

    #[rstest]
    #[case::existing_synthetic(am_asset(), 1_000_000, Ok(Cmd::burn(am_asset(), 1_000_000)))]
    #[case::non_existing_synthetic(phantom_asset(), 1_000_000, Err(Error::SyntheticNotFound))]
    fn burn(
        mut ctx: Context,
        #[case] synthetic: Synthetic,
        #[case] amount: SyntheticAmount,
        #[case] expected: Result<Cmd, Error>,
    ) {
        let actual = make_mint(&ctx).burn(synthetic, amount);

        assert_eq!(actual, expected);

        if let Ok(cmd) = actual {
            ctx.handle_cmd(cmd)
        }
    }
}
