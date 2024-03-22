use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_json_binary, Api, Binary, MessageInfo, StdError, Storage, SubMsg, Uint128};

use amulet_core::{
    admin::Repository as AdminRepository,
    mint::{
        mint, Cmd, ConfigCmd, Error as CoreMintError, Mint, MintCmd, Minter,
        Repository as CoreMintRepository, Synthetic, SyntheticAmount, Ticker,
    },
    Decimals, Recipient,
};

use crate::{
    admin::{get_admin_role, Error as AdminError},
    one_coin, PaymentError, StorageExt as _,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    CoreMint(#[from] CoreMintError),
    #[error(transparent)]
    CosmWasm(#[from] StdError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    Admin(#[from] AdminError),
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Create a synthetic so that it can be minted
    CreateSynthetic {
        /// The ticker of the synthetic, e.g. amuatom
        ticker: String,
        /// The number of decimals the synthetic uses
        decimals: u32,
    },
    /// Set minter's whitelist status
    SetWhitelisted { minter: String, whitelisted: bool },
    /// Mint an amount of synthetics to a recipient's address
    Mint {
        synthetic: String,
        amount: Uint128,
        recipient: String,
    },
    /// Burn the synthetics sent with this message
    Burn {},
}

#[cw_serde]
pub struct WhitelistedResponse {
    pub whitelisted: bool,
}

#[cw_serde]
pub struct SyntheticMetadataResponse {
    pub ticker: String,
    pub decimals: u32,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(WhitelistedResponse)]
    Whitelisted { minter: String },
    #[returns(SyntheticMetadataResponse)]
    SyntheticMetadata { synthetic: String },
}

pub fn handle_execute_msg(
    api: &dyn Api,
    admin_repository: &dyn AdminRepository,
    mint_repository: &dyn CoreMintRepository,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Cmd, Error> {
    let mint = mint(mint_repository);

    let cmd = match msg {
        ExecuteMsg::CreateSynthetic { ticker, decimals } => {
            let admin_role = get_admin_role(admin_repository, info)?;

            mint.create_synthetic(admin_role, ticker.into(), decimals)?
        }

        ExecuteMsg::SetWhitelisted {
            minter,
            whitelisted,
        } => {
            api.addr_validate(&minter)?;

            let admin_role = get_admin_role(admin_repository, info)?;

            mint.set_whitelisted(admin_role, minter.into(), whitelisted)?
        }

        ExecuteMsg::Mint {
            synthetic,
            amount,
            recipient,
        } => {
            api.addr_validate(&recipient)?;

            mint.mint(
                info.sender.into_string().into(),
                synthetic.into(),
                amount.u128(),
                recipient.into(),
            )?
        }

        ExecuteMsg::Burn {} => {
            let coin = one_coin(&info)?;

            mint.burn(coin.denom.into(), coin.amount.u128())?
        }
    };

    Ok(cmd)
}

pub fn handle_query_msg(storage: &dyn Storage, msg: QueryMsg) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::Whitelisted { minter } => {
            let whitelisted = Repository(storage)
                .is_whitelisted(&minter.into())
                .unwrap_or_default();

            to_json_binary(&WhitelistedResponse { whitelisted })
        }

        QueryMsg::SyntheticMetadata { synthetic } => {
            let ticker = storage
                .string_at(key::TICKER.with(&synthetic))
                .ok_or(StdError::not_found("synthetic"))?;

            let decimals = storage
                .u32_at(key::DECIMALS.with(&synthetic))
                .ok_or(StdError::not_found("synthetic"))?;

            to_json_binary(&SyntheticMetadataResponse { ticker, decimals })
        }
    }
}

pub struct Repository<'a>(&'a dyn Storage);

impl<'a> Repository<'a> {
    pub fn new(storage: &'a dyn Storage) -> Self {
        Self(storage)
    }
}

#[rustfmt::skip]
mod key {
    use crate::MapKey;
    
    macro_rules! key {
        ($k:literal) => {
            concat!("mint::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            crate::MapKey::new(key!($k))
        };
    }

    pub const SYNTHETIC : MapKey = map_key!("synthetic");
    pub const TICKER    : MapKey = map_key!("ticker");
    pub const DECIMALS  : MapKey = map_key!("decimals");
    pub const WHITELIST : MapKey = map_key!("whitelist");
}

impl<'a> CoreMintRepository for Repository<'a> {
    fn ticker_exists(&self, ticker: &Ticker) -> bool {
        self.0.has_key(key::SYNTHETIC.with(ticker))
    }

    fn synthetic_exists(&self, synthetic: &Synthetic) -> bool {
        self.0.has_key(key::TICKER.with(synthetic))
    }

    fn is_whitelisted(&self, minter: &Minter) -> Option<bool> {
        self.0.bool_at(key::WHITELIST.with(minter))
    }
}

pub trait TokenFactory<Msg> {
    fn denom(&self, ticker: &Ticker) -> String;

    fn create(&self, ticker: Ticker, decimals: Decimals) -> SubMsg<Msg>;

    fn mint(
        &self,
        synthetic: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> SubMsg<Msg>;

    fn burn(&self, synthetic: Synthetic, amount: SyntheticAmount) -> SubMsg<Msg>;
}

pub fn handle_cmd<Msg>(
    storage: &mut dyn Storage,
    token_factory: impl TokenFactory<Msg>,
    cmd: Cmd,
) -> Option<SubMsg<Msg>> {
    let msg = match cmd {
        Cmd::Config(cfg_cmd) => match cfg_cmd {
            ConfigCmd::CreateSynthetic { ticker, decimals } => {
                let denom = token_factory.denom(&ticker);

                storage.set_string(key::SYNTHETIC.with(&ticker), &denom);

                storage.set_string(key::TICKER.with(&denom), &ticker);

                storage.set_u32(key::DECIMALS.with(&denom), decimals);

                token_factory.create(ticker, decimals)
            }

            ConfigCmd::Whitelist { minter, enabled } => {
                storage.set_bool(key::WHITELIST.with(minter), enabled);

                return None;
            }
        },

        Cmd::Mint(mint_cmd) => match mint_cmd {
            MintCmd::Mint {
                synthetic,
                amount,
                recipient,
            } => token_factory.mint(synthetic, amount, recipient),

            MintCmd::Burn { synthetic, amount } => token_factory.burn(synthetic, amount),
        },
    };

    Some(msg)
}
