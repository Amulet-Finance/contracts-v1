use amulet_core::{
    mint::{Synthetic, SyntheticAmount, Ticker},
    Decimals, Recipient,
};
use amulet_cw::mint::TokenFactory as CwTokenFactory;
use cosmos_sdk_proto::cosmos::{
    bank::v1beta1::{DenomUnit, Metadata},
    base::v1beta1::Coin,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, CosmosMsg, Env};
use prost::Message;

#[cw_serde]
#[repr(u8)]
#[derive(Copy)]
pub enum Flavour {
    Osmosis = 1,
}

impl From<Flavour> for u8 {
    fn from(value: Flavour) -> Self {
        value as Self
    }
}

impl From<u8> for Flavour {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Osmosis,
            _ => panic!("{value} does not represent a token factory flavour"),
        }
    }
}

#[derive(prost::Message)]
struct OsmosisMsgCreateDenom {
    #[prost(string, tag = "1")]
    sender: String,
    #[prost(string, tag = "2")]
    subdenom: String,
}

#[derive(prost::Message)]
pub struct OsmosisMsgSetDenomMetadata {
    #[prost(string, tag = "1")]
    pub sender: String,
    #[prost(message, optional, tag = "2")]
    pub metadata: Option<Metadata>,
}

#[derive(prost::Message)]
struct OsmosisMsgMint {
    #[prost(string, tag = "1")]
    pub sender: String,
    #[prost(message, optional, tag = "2")]
    pub amount: Option<Coin>,
    #[prost(string, tag = "3")]
    pub mint_to_address: String,
}

#[derive(prost::Message)]
struct OsmosisMsgBurn {
    #[prost(string, tag = "1")]
    pub sender: String,
    #[prost(message, optional, tag = "2")]
    pub amount: Option<Coin>,
    #[prost(string, tag = "3")]
    pub burn_from_address: String,
}

#[derive(Clone)]
struct Denom(String);

impl Denom {
    fn new(contract_address: &str, subdenom: &str) -> Self {
        Self(format!("factory/{contract_address}/{subdenom}"))
    }
}

impl From<Denom> for String {
    fn from(value: Denom) -> Self {
        value.0
    }
}

fn bank_metadata(denom: Denom, display: &str, decimals: u32) -> Metadata {
    Metadata {
        description: "".to_owned(),
        denom_units: vec![
            DenomUnit {
                denom: denom.clone().into(),
                exponent: 0,
                aliases: vec![],
            },
            DenomUnit {
                denom: display.to_owned(),
                exponent: decimals,
                aliases: vec![],
            },
        ],
        base: denom.into(),
        display: display.to_owned(),
        name: display.to_owned(),
        symbol: display.to_owned(),
        uri: String::new(),
        uri_hash: String::new(),
    }
}

impl Flavour {
    pub const fn into_factory(self, env: &Env) -> TokenFactory {
        TokenFactory { flavour: self, env }
    }

    const fn create_denom_type_url(&self) -> &str {
        match self {
            Flavour::Osmosis => "/osmosis.tokenfactory.v1beta1.MsgCreateDenom",
        }
    }

    fn create_denom_msg(&self, sender: &str, subdenom: &str) -> Binary {
        match self {
            Flavour::Osmosis => OsmosisMsgCreateDenom {
                sender: sender.to_owned(),
                subdenom: subdenom.to_owned(),
            }
            .encode_to_vec()
            .into(),
        }
    }

    const fn set_metadata_type_url(&self) -> &str {
        match self {
            Flavour::Osmosis => "/osmosis.tokenfactory.v1beta1.MsgSetDenomMetadata",
        }
    }

    fn set_metadata_msg(&self, sender: &str, denom: Denom, display: &str, decimals: u32) -> Binary {
        match self {
            Flavour::Osmosis => OsmosisMsgSetDenomMetadata {
                sender: sender.to_owned(),
                metadata: Some(bank_metadata(denom, display, decimals)),
            }
            .encode_to_vec()
            .into(),
        }
    }

    const fn mint_type_url(&self) -> &str {
        match self {
            Flavour::Osmosis => "/osmosis.tokenfactory.v1beta1.MsgMint",
        }
    }

    fn mint_msg(&self, sender: &str, recipient: &str, denom: &str, amount: u128) -> Binary {
        match self {
            Flavour::Osmosis => OsmosisMsgMint {
                sender: sender.to_owned(),
                amount: Some(Coin {
                    denom: denom.into(),
                    amount: amount.to_string(),
                }),
                mint_to_address: recipient.to_owned(),
            }
            .encode_to_vec()
            .into(),
        }
    }

    const fn burn_type_url(&self) -> &str {
        match self {
            Flavour::Osmosis => "/osmosis.tokenfactory.v1beta1.MsgBurn",
        }
    }

    fn burn_msg(&self, sender: &str, denom: &str, amount: u128) -> Binary {
        match self {
            Flavour::Osmosis => OsmosisMsgBurn {
                sender: sender.to_owned(),
                amount: Some(Coin {
                    denom: denom.into(),
                    amount: amount.to_string(),
                }),
                burn_from_address: sender.to_owned(),
            }
            .encode_to_vec()
            .into(),
        }
    }
}

pub struct TokenFactory<'a> {
    flavour: Flavour,
    env: &'a Env,
}

impl TokenFactory<'_> {
    pub fn create_token<Msg>(&self, denom: &str) -> CosmosMsg<Msg> {
        CosmosMsg::Stargate {
            type_url: self.flavour.create_denom_type_url().to_owned(),
            value: self
                .flavour
                .create_denom_msg(self.env.contract.address.as_str(), denom),
        }
    }

    pub fn set_metadata<Msg>(&self, denom: &str, display: &str, decimals: u32) -> CosmosMsg<Msg> {
        CosmosMsg::Stargate {
            type_url: self.flavour.set_metadata_type_url().to_owned(),
            value: self.flavour.set_metadata_msg(
                self.env.contract.address.as_str(),
                Denom::new(self.env.contract.address.as_str(), denom),
                display,
                decimals,
            ),
        }
    }

    pub fn mint<Msg>(&self, denom: &str, recipient: &str, amount: u128) -> CosmosMsg<Msg> {
        CosmosMsg::Stargate {
            type_url: self.flavour.mint_type_url().to_owned(),
            value: self.flavour.mint_msg(
                self.env.contract.address.as_str(),
                recipient,
                denom,
                amount,
            ),
        }
    }

    pub fn burn<Msg>(&self, denom: &str, amount: u128) -> CosmosMsg<Msg> {
        CosmosMsg::Stargate {
            type_url: self.flavour.burn_type_url().to_owned(),
            value: self
                .flavour
                .burn_msg(self.env.contract.address.as_str(), denom, amount),
        }
    }
}

impl<Msg> CwTokenFactory<Msg> for TokenFactory<'_> {
    fn denom(&self, ticker: &Ticker) -> String {
        Denom::new(self.env.contract.address.as_str(), ticker.as_str()).into()
    }

    fn create(&self, ticker: Ticker) -> CosmosMsg<Msg> {
        self.create_token(ticker.as_str())
    }

    fn set_metadata(&self, ticker: &Ticker, decimals: Decimals) -> CosmosMsg<Msg> {
        TokenFactory::set_metadata(self, ticker.as_str(), ticker.display(), decimals)
    }

    fn mint(
        &self,
        denom: Synthetic,
        amount: SyntheticAmount,
        recipient: Recipient,
    ) -> CosmosMsg<Msg> {
        TokenFactory::mint(self, denom.as_str(), recipient.as_str(), amount)
    }

    fn burn(&self, denom: Synthetic, amount: SyntheticAmount) -> CosmosMsg<Msg> {
        TokenFactory::burn(self, denom.as_str(), amount)
    }
}
