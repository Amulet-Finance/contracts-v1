use cosmwasm_std::{
    coins, to_json_binary, CustomQuery, QuerierWrapper, StdError, Storage, SubMsg, WasmMsg,
};

use amulet_core::{
    hub::{
        AdvanceFee, Amo, AmoAllocation, CollateralYieldFee, MaxLtv, Oracle, Proxy, ReserveYieldFee,
        VaultCmd, VaultDepositReason, VaultId, Vaults as CoreVaults,
    },
    mint::Synthetic,
    vault::{DepositAmount, SharesAmount, TotalDepositsValue, TotalSharesIssued},
    Asset, Decimals, Recipient,
};

use crate::{
    vault::{
        DepositAssetResponse, ExecuteMsg as VaultExecuteMsg, QueryMsg as VaultQueryMsg,
        SharesAssetResponse, StateResponse, UnderlyingAssetDecimalsResponse,
    },
    StorageExt as _,
};

pub const DEPOSIT_REPLY_ID: u64 = VaultDepositReason::Deposit as u64;
pub const REPAY_UNDERLYING_REPLY_ID: u64 = VaultDepositReason::RepayUnderlying as u64;
pub const MINT_REPLY_ID: u64 = VaultDepositReason::Mint as u64;

pub struct Vaults<'a> {
    storage: &'a dyn Storage,
    querier: QuerierWrapper<'a>,
}

impl<'a> Vaults<'a> {
    pub fn new(storage: &'a dyn Storage, querier: QuerierWrapper<'a, impl CustomQuery>) -> Self {
        Self {
            storage,
            querier: querier.into_empty(),
        }
    }
}

#[rustfmt::skip]
mod key {
    use crate::MapKey;

    macro_rules! key {
        ($k:literal) => {
            concat!("hub_vaults::", $k)
        };
    }

    macro_rules! map_key {
        ($k:literal) => {
            crate::MapKey::new(key!($k))
        };
    }

    pub const SYNTHETICS               : MapKey = map_key!("synthetics");
    pub const DEPOSITS_ENABLED         : MapKey = map_key!("deposits_enabled");
    pub const ADVANCE_ENABLED          : MapKey = map_key!("advance_enabled");
    pub const MAX_LTV                  : MapKey = map_key!("max_ltv");
    pub const COLLATERAL_YIELD_FEE     : MapKey = map_key!("collateral_yield_fee");
    pub const RESERVE_YIELD_FEE        : MapKey = map_key!("reserve_yield_fee");
    pub const FIXED_ADVANCE_FEE        : MapKey = map_key!("fixed_advance_fee");
    pub const ADVANCE_FEE_RECIPIENT    : MapKey = map_key!("advance_fee_recipient");
    pub const ADVANCE_FEE_ORACLE       : MapKey = map_key!("advance_fee_oracle");
    pub const AMO                      : MapKey = map_key!("amo");
    pub const AMO_ALLOCATION           : MapKey = map_key!("amo_allocation");
    pub const DEPOSIT_PROXY            : MapKey = map_key!("deposit_proxy");
    pub const ADVANCE_PROXY            : MapKey = map_key!("advance_proxy");
    pub const REDEEM_PROXY             : MapKey = map_key!("redeem_proxy");
    pub const MINT_PROXY               : MapKey = map_key!("mint_proxy");
    pub const VAULT_ADDRESS            : MapKey = map_key!("vault_address");
    pub const VAULT_COUNT              : &str   = key!("vault_count");
    pub const VAULT_CALLBACK_VAULT     : &str   = key!("vault_callback_vault");
    pub const VAULT_CALLBACK_RECIPIENT : &str   = key!("vault_callback_recipient");
}

pub trait StorageExt: Storage {
    fn vault_count(&self) -> u32 {
        self.u32_at(key::VAULT_COUNT).unwrap_or_default()
    }

    fn vault_address(&self, index: u32) -> Option<String> {
        self.string_at(key::VAULT_ADDRESS.with(index))
    }

    fn vault_callback_vault(&self) -> String {
        self.string_at(key::VAULT_CALLBACK_VAULT)
            .expect("always: set before vault msg issued")
    }

    fn vault_callback_recipient(&self) -> String {
        self.string_at(key::VAULT_CALLBACK_RECIPIENT)
            .expect("always: set before vault msg issued")
    }

    fn add_vault_address(&mut self, address: &str) {
        let count = self.vault_count();

        self.set_string(key::VAULT_ADDRESS.with(count), address);

        self.set_u32(key::VAULT_COUNT, count + 1);
    }

    fn set_vault_callback_vault(&mut self, vault: &str) {
        self.set_string(key::VAULT_CALLBACK_VAULT, vault)
    }

    fn set_vault_callback_recipient(&mut self, recipient: &str) {
        self.set_string(key::VAULT_CALLBACK_RECIPIENT, recipient)
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

impl<'a> CoreVaults for Vaults<'a> {
    fn underlying_asset_decimals(&self, vault: &VaultId) -> Option<Decimals> {
        let response: UnderlyingAssetDecimalsResponse = self
            .querier
            .query_wasm_smart(vault.clone(), &VaultQueryMsg::UnderlyingAssetDecimals {})
            .map_err(|err| match err {
                err @ StdError::NotFound { .. } => err,
                err => panic!("{vault}: {err}"),
            })
            .ok()?;

        Some(response.decimals)
    }

    fn is_registered(&self, vault: &VaultId) -> bool {
        self.storage
            .get(key::SYNTHETICS.with(vault).as_bytes())
            .is_some()
    }

    fn deposits_enabled(&self, vault: &VaultId) -> bool {
        self.storage
            .bool_at(key::DEPOSITS_ENABLED.with(vault))
            .unwrap_or_default()
    }

    fn advance_enabled(&self, vault: &VaultId) -> bool {
        self.storage
            .bool_at(key::ADVANCE_ENABLED.with(vault))
            .unwrap_or_default()
    }

    fn max_ltv(&self, vault: &VaultId) -> Option<MaxLtv> {
        self.storage
            .u32_at(key::MAX_LTV.with(vault))
            .and_then(MaxLtv::new)
    }

    fn collateral_yield_fee(&self, vault: &VaultId) -> Option<CollateralYieldFee> {
        self.storage
            .u32_at(key::COLLATERAL_YIELD_FEE.with(vault))
            .and_then(CollateralYieldFee::new)
    }

    fn reserve_yield_fee(&self, vault: &VaultId) -> Option<ReserveYieldFee> {
        self.storage
            .u32_at(key::RESERVE_YIELD_FEE.with(vault))
            .and_then(ReserveYieldFee::new)
    }

    fn fixed_advance_fee(&self, vault: &VaultId) -> Option<AdvanceFee> {
        self.storage
            .u32_at(key::FIXED_ADVANCE_FEE.with(vault))
            .and_then(AdvanceFee::new)
    }

    fn advance_fee_recipient(&self, vault: &VaultId) -> Option<Recipient> {
        self.storage
            .string_at(key::ADVANCE_FEE_RECIPIENT.with(vault))
            .map(Recipient::from)
    }

    fn advance_fee_oracle(&self, vault: &VaultId) -> Option<Oracle> {
        self.storage
            .string_at(key::ADVANCE_FEE_ORACLE.with(vault))
            .map(Oracle::from)
    }

    fn amo(&self, vault: &VaultId) -> Option<Amo> {
        self.storage.string_at(key::AMO.with(vault)).map(Amo::from)
    }

    fn amo_allocation(&self, vault: &VaultId) -> Option<AmoAllocation> {
        self.storage
            .u32_at(key::AMO_ALLOCATION.with(vault))
            .and_then(AmoAllocation::new)
    }

    fn deposit_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.storage
            .string_at(key::DEPOSIT_PROXY.with(vault))
            .map(Proxy::from)
    }

    fn advance_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.storage
            .string_at(key::ADVANCE_PROXY.with(vault))
            .map(Proxy::from)
    }

    fn redeem_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.storage
            .string_at(key::REDEEM_PROXY.with(vault))
            .map(Proxy::from)
    }

    fn mint_proxy(&self, vault: &VaultId) -> Option<Proxy> {
        self.storage
            .string_at(key::MINT_PROXY.with(vault))
            .map(Proxy::from)
    }

    fn deposit_asset(&self, vault: &VaultId) -> Asset {
        let response: DepositAssetResponse = match self
            .querier
            .query_wasm_smart(vault.clone(), &VaultQueryMsg::DepositAsset {})
        {
            Ok(response) => response,
            Err(err) => panic!("deposit asset query failed: {err} - {vault}"),
        };

        response.denom.into()
    }

    fn shares_asset(&self, vault: &VaultId) -> Asset {
        let response: SharesAssetResponse = match self
            .querier
            .query_wasm_smart(vault.clone(), &VaultQueryMsg::SharesAsset {})
        {
            Ok(response) => response,
            Err(err) => panic!("shares asset query failed: {err} - {vault}"),
        };

        response.denom.into()
    }

    fn synthetic_asset(&self, vault: &VaultId) -> Synthetic {
        self.storage
            .string_at(key::SYNTHETICS.with(vault))
            .map(Synthetic::from)
            .expect("vault has been registered")
    }

    fn total_shares_issued(&self, vault: &VaultId) -> TotalSharesIssued {
        let response: StateResponse = match self
            .querier
            .query_wasm_smart(vault.clone(), &VaultQueryMsg::State {})
        {
            Ok(response) => response,
            Err(err) => panic!("state query failed: {err} - {vault}"),
        };

        TotalSharesIssued(response.total_issued_shares.u128())
    }

    fn total_deposits_value(&self, vault: &VaultId) -> TotalDepositsValue {
        let response: StateResponse = match self
            .querier
            .query_wasm_smart(vault.clone(), &VaultQueryMsg::State {})
        {
            Ok(response) => response,
            Err(err) => panic!("state query failed: {err} - {vault}"),
        };

        TotalDepositsValue(response.total_deposits.u128())
    }
}

pub fn handle_cmd<Msg>(storage: &mut dyn Storage, cmd: VaultCmd) -> Option<SubMsg<Msg>> {
    match cmd {
        VaultCmd::Register { vault, synthetic } => {
            storage.add_vault_address(&vault);
            storage.set_string(key::SYNTHETICS.with(vault), &synthetic);
        }

        VaultCmd::SetDepositsEnabled { vault, enabled } => {
            storage.set_bool(key::DEPOSITS_ENABLED.with(vault), enabled);
        }

        VaultCmd::SetAdvanceEnabled { vault, enabled } => {
            storage.set_bool(key::ADVANCE_ENABLED.with(vault), enabled);
        }

        VaultCmd::SetMaxLtv { vault, max_ltv } => {
            storage.set_u32(key::MAX_LTV.with(vault), max_ltv.raw());
        }

        VaultCmd::SetCollateralYieldFee { vault, fee } => {
            storage.set_u32(key::COLLATERAL_YIELD_FEE.with(vault), fee.raw());
        }

        VaultCmd::SetReserveYieldFee { vault, fee } => {
            storage.set_u32(key::RESERVE_YIELD_FEE.with(vault), fee.raw());
        }

        VaultCmd::SetAdvanceFeeRecipient { vault, recipient } => {
            storage.set_string(key::ADVANCE_FEE_RECIPIENT.with(vault), &recipient);
        }

        VaultCmd::SetFixedAdvanceFee { vault, fee } => {
            storage.set_u32(key::FIXED_ADVANCE_FEE.with(vault), fee.raw());
        }

        VaultCmd::SetAdvanceFeeOracle { vault, oracle } => {
            storage.set_string(key::ADVANCE_FEE_ORACLE.with(vault), &oracle);
        }

        VaultCmd::SetAmo { vault, amo } => {
            storage.set_string(key::AMO.with(vault), &amo);
        }

        VaultCmd::SetAmoAllocation { vault, allocation } => {
            storage.set_u32(key::AMO_ALLOCATION.with(vault), allocation.raw());
        }

        VaultCmd::SetDepositProxy { vault, proxy } => {
            storage.set_string(key::DEPOSIT_PROXY.with(vault), &proxy);
        }

        VaultCmd::SetAdvanceProxy { vault, proxy } => {
            storage.set_string(key::ADVANCE_PROXY.with(vault), &proxy);
        }

        VaultCmd::SetRedeemProxy { vault, proxy } => {
            storage.set_string(key::REDEEM_PROXY.with(vault), &proxy);
        }

        VaultCmd::SetMintProxy { vault, proxy } => {
            storage.set_string(key::MINT_PROXY.with(vault), &proxy);
        }

        VaultCmd::Deposit {
            vault,
            asset,
            amount: DepositAmount(amount),
            callback_recipient,
            callback_reason,
        } => {
            // cache callback details
            storage.set_vault_callback_vault(&vault);
            storage.set_vault_callback_recipient(&callback_recipient);

            let msg = WasmMsg::Execute {
                contract_addr: vault.into_string(),
                msg: to_json_binary(&VaultExecuteMsg::Deposit {})
                    .expect("infallible serialization"),
                funds: coins(amount, asset),
            };

            let reply_id = match callback_reason {
                VaultDepositReason::Deposit => DEPOSIT_REPLY_ID,
                VaultDepositReason::RepayUnderlying => REPAY_UNDERLYING_REPLY_ID,
                VaultDepositReason::Mint => MINT_REPLY_ID,
            };

            return Some(SubMsg::reply_on_success(msg, reply_id));
        }

        VaultCmd::Redeem {
            vault,
            shares,
            amount: SharesAmount(amount),
            recipient,
        } => {
            let msg = WasmMsg::Execute {
                contract_addr: vault.into_string(),
                msg: to_json_binary(&VaultExecuteMsg::Redeem {
                    recipient: recipient.into_string(),
                })
                .expect("infallible serialization"),
                funds: coins(amount, shares),
            };

            return Some(SubMsg::new(msg));
        }
    }

    None
}
