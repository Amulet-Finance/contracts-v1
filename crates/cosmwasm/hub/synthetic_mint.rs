use cosmwasm_std::{
    coins, to_json_binary, Api, CustomQuery, QuerierWrapper, StdError, Storage, SubMsg, WasmMsg,
};

use amulet_core::{
    hub::SyntheticMint as CoreSyntheticMint,
    mint::{MintCmd, Synthetic},
    Decimals,
};

use crate::{
    mint::{ExecuteMsg as MintExecuteMsg, Metadata, QueryMsg as MintQueryMsg},
    StorageExt as _,
};

pub struct SyntheticMint<'a> {
    storage: &'a dyn Storage,
    querier: QuerierWrapper<'a>,
}

impl<'a> SyntheticMint<'a> {
    pub fn new(storage: &'a dyn Storage, querier: QuerierWrapper<'a, impl CustomQuery>) -> Self {
        Self {
            storage,
            querier: querier.into_empty(),
        }
    }
}

#[rustfmt::skip]
mod key {
    macro_rules! key {
        ($k:literal) => {
            concat!("hub_synthetic_mint::", $k)
        };
    }

    pub const MINT_ADDRESS : &str = key!("mint_address");
}

pub trait StorageExt: Storage {
    fn mint_address(&self) -> String {
        self.string_at(key::MINT_ADDRESS)
            .expect("assumed: set during initialisation")
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

pub fn init(api: &dyn Api, storage: &mut dyn Storage, mint_address: &str) -> Result<(), StdError> {
    api.addr_validate(mint_address)?;

    storage.set_string(key::MINT_ADDRESS, mint_address);

    Ok(())
}

impl<'a> CoreSyntheticMint for SyntheticMint<'a> {
    fn syntethic_decimals(&self, synthetic: &Synthetic) -> Option<Decimals> {
        let mint = self.storage.mint_address();

        let query_result: Result<Metadata, _> = self.querier.query_wasm_smart(
            mint,
            &MintQueryMsg::Synthetic {
                denom: synthetic.to_string(),
            },
        );

        match query_result {
            Ok(res) => Some(res.decimals),
            Err(err) => match err {
                StdError::NotFound { .. } => None,
                err => panic!("unexpected error querying mint: {err}"),
            },
        }
    }
}

pub fn handle_cmd<Msg>(storage: &dyn Storage, cmd: MintCmd) -> SubMsg<Msg> {
    let mint = storage.mint_address();

    let msg = match cmd {
        MintCmd::Mint {
            synthetic,
            amount,
            recipient,
        } => WasmMsg::Execute {
            contract_addr: mint,
            msg: to_json_binary(&MintExecuteMsg::Mint {
                synthetic: synthetic.into_string(),
                amount: amount.into(),
                recipient: recipient.into_string(),
            })
            .expect("infallible serialization"),
            funds: vec![],
        },

        MintCmd::Burn { synthetic, amount } => WasmMsg::Execute {
            contract_addr: mint,
            msg: to_json_binary(&MintExecuteMsg::Burn {}).expect("infallible serialization"),
            funds: coins(amount, synthetic),
        },
    };

    SubMsg::new(msg)
}
