pub mod advance_fee_oracle;
pub mod balance_sheet;
pub mod synthetic_mint;
pub mod vaults;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, Binary, Env, MessageInfo, Reply, Response, StdError,
    Storage, Uint128, Uint256,
};

use amulet_core::{
    admin::Repository as AdminRepository,
    hub::{
        configure, hub, positions::update_cdp, Account, AdvanceFeeOracle as CoreAdvanceFeeOracle,
        BalanceSheet as CoreBalanceSheet, Cdp, Cmd, ConfigureHub, Error as CoreHubError, Hub,
        ProxyConfig, SyntheticMint as CoreSyntheticMint, VaultDepositReason, VaultId,
        Vaults as CoreVaults,
    },
    Identifier,
};
use cw_utils::{one_coin, parse_reply_execute_data, ParseReplyError, PaymentError};

use crate::{
    admin::{get_admin_role, Error as AdminError},
    vault::DepositResponse as VaultDepositResponse,
};

use self::{
    balance_sheet::StorageExt as _,
    vaults::{StorageExt as _, DEPOSIT_REPLY_ID, MINT_REPLY_ID, REPAY_UNDERLYING_REPLY_ID},
};

pub use self::{
    advance_fee_oracle::AdvanceFeeOracle,
    balance_sheet::{handle_cmd as handle_balance_sheet_cmd, BalanceSheet},
    synthetic_mint::{handle_cmd as handle_mint_cmd, init as init_mint, SyntheticMint},
    vaults::{handle_cmd as handle_vault_cmd, Vaults},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    CoreHub(#[from] CoreHubError),
    #[error(transparent)]
    CosmWasm(#[from] StdError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    Admin(#[from] AdminError),
    #[error(transparent)]
    Reply(#[from] ParseReplyError),
}

#[cw_serde]
pub enum AdminMsg {
    /// Register a vault, making it available for deposits
    RegisterVault {
        /// The address of the vault contract
        vault: String,
        /// The synthetic to be associated with the vault
        synthetic: String,
    },
    /// Set the treasury
    SetTreasury { address: String },
    /// Set the enabled status of deposits for the vault
    SetDepositsEnabled { vault: String, enabled: bool },
    /// Set the enabled status of advance for the vault
    SetAdvanceEnabled { vault: String, enabled: bool },
    /// Set the max LTV allowed for the vault
    SetMaxLtv { vault: String, bps: u32 },
    /// Set the treasury fee to be applied to yield earned on collateral
    SetCollateralYieldFee { vault: String, bps: u32 },
    /// Set the treasury fee to be applied to yield earned on reserves
    SetReservesTreasuryFee { vault: String, bps: u32 },
    /// Set the advance fee recipient for the vault
    SetAdvanceFeeRecipient { vault: String, recipient: String },
    /// Set the fixed advance fee to be used for the vault if no oracle is set
    SetFixedAdvanceFee { vault: String, bps: u32 },
    /// Set the advance fee oracle for the vault
    SetAdvanceFeeOracle { vault: String, oracle: String },
    /// Set the 'Automatic Market Operator' (AMO) for the vault
    SetAmo { vault: String, amo: String },
    /// Set the AMO allocation to be used for the vault
    SetAmoAllocation { vault: String, bps: u32 },
    /// Set the proxy configuration to be used for the vault
    SetProxyConfig {
        /// The vault to set the proxy configuration for
        vault: String,
        /// The deposit proxy address to set, if any
        deposit: Option<String>,
        /// The advance proxy address to set, if any
        advance: Option<String>,
        /// The redeem proxy address to set, if any
        redeem: Option<String>,
        /// The mint proxy address to set, if any
        mint: Option<String>,
    },
}

#[cw_serde]
pub enum UserMsg {
    // Messages for Account Positions
    /// Evaluate a vault, progressing any payments
    /// Responds with [PositionResponse]
    Evaluate { vault: String },
    /// Deposit native token into a vault
    /// Responds with [PositionResponse]
    Deposit { vault: String },
    /// Deposit native token into a vault on behalf of another (proxied deposit)
    /// Responds with [PositionResponse]
    DepositOnBehalf { vault: String, behalf_of: String },
    /// Repay debt against a vault using the underlying token
    /// Responds with [PositionResponse]
    RepayUnderlying { vault: String },
    /// Repay debt against a vault using the synthetic token
    /// Responds with [PositionResponse]
    RepaySynthetic { vault: String },
    /// Request an advance against a vault deposit
    /// Responds with [PositionResponse]
    Advance { vault: String, amount: Uint128 },
    /// Request an advance on behalf of another against their vault deposit (proxied advance)
    /// Responds with [PositionResponse]
    AdvanceOnBehalf {
        vault: String,
        amount: Uint128,
        behalf_of: String,
    },
    /// Request to withdraw funds from a vault
    /// Responds with [PositionResponse]
    Withdraw { vault: String, amount: Uint128 },
    /// Request to self-liquidate a vault position
    SelfLiquidate { vault: String },
    /// Request to convert a `vault` position's credit into collateral
    ConvertCredit { vault: String, amount: Uint128 },
    /// Redeem synthetics for reserve holdings
    Redeem { vault: String },
    /// Redeem synthetics for reserve holdings on behalf of another (proxied mint)
    RedeemOnBehalf { vault: String, behalf_of: String },
    /// Mint synthetics for depositing native token into a vault
    Mint { vault: String },
    /// Mint synthetics for depositing native token into a vault on behalf of another (proxied mint)
    MintOnBehalf { vault: String, behalf_of: String },
    /// Request to claim the treasury's accumulated `vault` shares
    ClaimTreasury { vault: String },
    /// Request to claim the AMO's accumulated `vault` shares
    ClaimAmo { vault: String },
}

#[cw_serde]
#[serde(untagged)]
pub enum ExecuteMsg {
    Admin(AdminMsg),
    User(UserMsg),
}

#[cw_serde]
#[derive(Default)]
pub struct PositionResponse {
    /// Amount of collateral depositted in the vault
    pub collateral: Uint128,
    /// Amount of matched assets advanced against the vault
    pub debt: Uint128,
    /// Amount of credit owed to the account
    pub credit: Uint128,
    /// The Sum Payment Ratio at the time of position evaluation
    pub sum_payment_ratio: Uint256,
    /// Whether or not there was a vault loss detected.
    /// If `true` the other fields will be based on the last stored overall SPR for the vault.
    pub vault_loss_detected: bool,
}

#[cw_serde]
pub struct SumPaymentRatio {
    pub ratio: Uint256,
    pub timestamp: u64,
}

#[cw_serde]
pub struct VaultMetadata {
    /// Address/Id of the vault
    pub vault: String,
    /// Denom of the associated synthetic (debt token)
    pub synthetic: String,
    /// The deposit enabled state
    pub deposit_enabled: bool,
    /// The advance enabled state
    pub advance_enabled: bool,
    /// The maximum Loan-To-Value (LTV) allowed in basis points
    pub max_ltv_bps: u32,
    /// The base fee applied to yield payments in basis points
    pub collateral_yield_fee_bps: u32,
    /// The fee applied to reserves yield payments in basis points
    pub reserve_yield_fee_bps: u32,
    /// The default fee applied to a requested advance amount in basis points (if there is no oracle set)
    pub fixed_advance_fee_bps: u32,
    /// The advance fee recipient associated with the vault, if any
    pub advance_fee_recipient: Option<String>,
    /// The advance fee rate oracle assigned to the vault, if any
    pub advance_fee_oracle: Option<String>,
    /// The total amount of deposited collateral
    pub collateral_balance: Uint128,
    /// The amount of vault shares representing deposited collateral
    pub collateral_shares: Uint128,
    /// The total amount of assets in the reserve
    pub reserve_balance: Uint128,
    /// The amount of vault shares representing the reserve balance
    pub reserve_shares: Uint128,
    /// The amount of shares claimable by the treasury
    pub treasury_shares: Uint128,
    /// The AMO associated with the vault, if any
    pub amo: Option<String>,
    /// The portion of payments allocated to the AMO
    pub amo_allocation: u32,
    /// The amount of shares claimable by the AMO
    pub amo_shares: Uint128,
    /// The on-going sum of payments over collateral, if any
    pub sum_payment_ratio: Option<SumPaymentRatio>,
    /// The address of the deposit proxy, if any
    pub deposit_proxy: Option<String>,
    /// The address of the advance proxy, if any
    pub advance_proxy: Option<String>,
    /// The address of the mint proxy, if any
    pub mint_proxy: Option<String>,
    /// The address of the redeem proxy, if any
    pub redeem_proxy: Option<String>,
}

#[cw_serde]
pub struct ListVaultsResponse {
    pub vaults: Vec<VaultMetadata>,
}

#[cw_serde]
pub struct TreasuryResponse {
    /// The address authorised to claim treasury allocations
    pub treasury: Option<String>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ListVaultsResponse)]
    ListVaults {},
    #[returns(VaultMetadata)]
    VaultMetadata { vault: String },
    #[returns(PositionResponse)]
    Position { account: String, vault: String },
    #[returns(TreasuryResponse)]
    Treasury {},
}

impl From<Cdp> for PositionResponse {
    fn from(cdp: Cdp) -> Self {
        Self {
            collateral: cdp.collateral.into(),
            debt: cdp.debt.into(),
            credit: cdp.credit.into(),
            sum_payment_ratio: Uint256::from_be_bytes(cdp.spr.into_raw().to_be_bytes()),
            vault_loss_detected: false,
        }
    }
}

trait ResponseExt: Sized {
    fn from_cdp(cdp: Cdp) -> Self;
}

impl<Msg> ResponseExt for Response<Msg> {
    fn from_cdp(cdp: Cdp) -> Self {
        let data = to_json_binary(&PositionResponse::from(cdp)).expect("infallible serialization");

        Self::default().set_data(data)
    }
}

fn handle_deposit<Msg>(
    hub: &dyn Hub,
    info: MessageInfo,
    vault: String,
    recipient: String,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let coin = one_coin(&info)?;

    let cmds = hub.deposit(
        vault.into(),
        info.sender.into_string().into(),
        coin.denom.into(),
        coin.amount.u128(),
        recipient.into(),
    )?;

    Ok((cmds, Response::default()))
}

fn handle_advance<Msg>(
    hub: &dyn Hub,
    info: MessageInfo,
    vault: String,
    recipient: String,
    amount: Uint128,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let cmds = hub.advance(
        vault.into(),
        info.sender.into_string().into(),
        amount.u128(),
        recipient.into(),
    )?;

    Ok((cmds, Response::default()))
}

fn handle_redeem<Msg>(
    hub: &dyn Hub,
    info: MessageInfo,
    vault: String,
    recipient: String,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let coin = one_coin(&info)?;

    let cmds = hub.redeem_synthetic(
        vault.into(),
        info.sender.into_string().into(),
        coin.denom.into(),
        coin.amount.u128(),
        recipient.into(),
    )?;

    Ok((cmds, Response::default()))
}

fn handle_mint<Msg>(
    hub: &dyn Hub,
    info: MessageInfo,
    vault: String,
    recipient: String,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let coin = one_coin(&info)?;

    let cmds = hub.mint_synthetic(
        vault.into(),
        info.sender.into_string().into(),
        coin.denom.into(),
        coin.amount.u128(),
        recipient.into(),
    )?;

    Ok((cmds, Response::default()))
}

pub fn handle_admin_msg<Msg>(
    api: &dyn Api,
    admin_repository: &dyn AdminRepository,
    vaults: &dyn CoreVaults,
    mint: &dyn CoreSyntheticMint,
    info: MessageInfo,
    msg: AdminMsg,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let admin_role = get_admin_role(admin_repository, &info)?;

    let config = configure(vaults, mint);

    let cmds = match msg {
        AdminMsg::RegisterVault { vault, synthetic } => {
            api.addr_validate(&vault)?;

            config.register_vault(admin_role, vault.into(), synthetic.into())?
        }

        AdminMsg::SetTreasury { address } => {
            api.addr_validate(&address)?;

            config.set_treasury(admin_role, address.into())?
        }

        AdminMsg::SetDepositsEnabled { vault, enabled } => {
            config.set_deposit_enabled(admin_role, vault.into(), enabled)?
        }

        AdminMsg::SetAdvanceEnabled { vault, enabled } => {
            config.set_advance_enabled(admin_role, vault.into(), enabled)?
        }

        AdminMsg::SetMaxLtv { vault, bps } => config.set_max_ltv(admin_role, vault.into(), bps)?,

        AdminMsg::SetCollateralYieldFee { vault, bps } => {
            config.set_collateral_yield_fee(admin_role, vault.into(), bps)?
        }

        AdminMsg::SetReservesTreasuryFee { vault, bps } => {
            config.set_reserve_yield_fee(admin_role, vault.into(), bps)?
        }

        AdminMsg::SetAdvanceFeeRecipient { vault, recipient } => {
            api.addr_validate(&recipient)?;

            config.set_advance_fee_recipient(admin_role, vault.into(), recipient.into())?
        }

        AdminMsg::SetFixedAdvanceFee { vault, bps } => {
            config.set_fixed_advance_fee(admin_role, vault.into(), bps)?
        }

        AdminMsg::SetAdvanceFeeOracle { vault, oracle } => {
            api.addr_validate(&oracle)?;

            config.set_advance_fee_oracle(admin_role, vault.into(), oracle.into())?
        }

        AdminMsg::SetAmo { vault, amo } => {
            api.addr_validate(&amo)?;

            config.set_amo(admin_role, vault.into(), amo.into())?
        }

        AdminMsg::SetAmoAllocation { vault, bps } => {
            config.set_amo_allocation(admin_role, vault.into(), bps)?
        }

        AdminMsg::SetProxyConfig {
            vault,
            deposit,
            advance,
            redeem,
            mint,
        } => {
            macro_rules! validate_proxy_addr {
                ($api:ident, $proxy:ident) => {
                    $proxy
                        .map(|p| $api.addr_validate(&p))
                        .transpose()?
                        .map(Addr::into_string)
                        .map(Into::into)
                };
            }

            config.set_proxy_config(
                admin_role,
                vault.into(),
                ProxyConfig {
                    deposit: validate_proxy_addr!(api, deposit),
                    advance: validate_proxy_addr!(api, advance),
                    redeem: validate_proxy_addr!(api, redeem),
                    mint: validate_proxy_addr!(api, mint),
                },
            )?
        }
    };

    Ok((cmds, Response::default()))
}

pub fn handle_user_msg<Msg>(
    api: &dyn Api,
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
    advance_fee_oracle: &dyn CoreAdvanceFeeOracle,
    info: MessageInfo,
    msg: UserMsg,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let hub = hub(vaults, balance_sheet, advance_fee_oracle);

    match msg {
        UserMsg::Evaluate { vault } => {
            let response = hub.evaluate(vault.into(), info.sender.into_string().into())?;

            Ok((response.cmds, Response::from_cdp(response.cdp)))
        }

        UserMsg::Deposit { vault } => {
            let recipient = info.sender.clone().into_string();

            handle_deposit(&hub, info, vault, recipient)
        }

        UserMsg::DepositOnBehalf { vault, behalf_of } => {
            api.addr_validate(&behalf_of)?;

            handle_deposit(&hub, info, vault, behalf_of)
        }

        UserMsg::RepayUnderlying { vault } => {
            let coin = one_coin(&info)?;

            let cmds = hub.repay_underlying(
                vault.into(),
                info.sender.into_string().into(),
                coin.denom.into(),
                coin.amount.u128(),
            )?;

            Ok((cmds, Response::default()))
        }

        UserMsg::RepaySynthetic { vault } => {
            let coin = one_coin(&info)?;

            let response = hub.repay_synthetic(
                vault.into(),
                info.sender.into_string().into(),
                coin.denom.into(),
                coin.amount.u128(),
            )?;

            Ok((response.cmds, Response::from_cdp(response.cdp)))
        }

        UserMsg::Advance { vault, amount } => {
            let recipient = info.sender.clone().into_string();

            handle_advance(&hub, info, vault, recipient, amount)
        }

        UserMsg::AdvanceOnBehalf {
            vault,
            amount,
            behalf_of,
        } => {
            api.addr_validate(&behalf_of)?;

            handle_advance(&hub, info, vault, behalf_of, amount)
        }

        UserMsg::Withdraw { vault, amount } => {
            let response = hub.withdraw_collateral(
                vault.into(),
                info.sender.into_string().into(),
                amount.u128(),
            )?;

            Ok((response.cmds, Response::from_cdp(response.cdp)))
        }

        UserMsg::SelfLiquidate { vault } => {
            let cmds =
                hub.self_liquidate_position(vault.into(), info.sender.into_string().into())?;

            Ok((cmds, Response::default()))
        }

        UserMsg::ConvertCredit { vault, amount } => {
            let response = hub.convert_credit(
                vault.into(),
                info.sender.into_string().into(),
                amount.u128(),
            )?;

            Ok((response.cmds, Response::from_cdp(response.cdp)))
        }

        UserMsg::Redeem { vault } => {
            let recipient = info.sender.clone().into_string();

            handle_redeem(&hub, info, vault, recipient)
        }

        UserMsg::RedeemOnBehalf { vault, behalf_of } => {
            api.addr_validate(&behalf_of)?;

            handle_redeem(&hub, info, vault, behalf_of)
        }

        UserMsg::Mint { vault } => {
            let recipient = info.sender.clone().into_string();

            handle_mint(&hub, info, vault, recipient)
        }

        UserMsg::MintOnBehalf { vault, behalf_of } => {
            api.addr_validate(&behalf_of)?;

            handle_mint(&hub, info, vault, behalf_of)
        }

        UserMsg::ClaimTreasury { vault } => {
            let cmds = hub.claim_treasury_shares(vault.into(), info.sender.into_string().into())?;

            Ok((cmds, Response::default()))
        }

        UserMsg::ClaimAmo { vault } => {
            let cmds = hub.claim_amo_shares(vault.into(), info.sender.into_string().into())?;

            Ok((cmds, Response::default()))
        }
    }
}

pub struct Ctx<'a> {
    pub api: &'a dyn Api,
    pub vaults: &'a dyn CoreVaults,
    pub admin_repository: &'a dyn AdminRepository,
    pub mint: &'a dyn CoreSyntheticMint,
    pub balance_sheet: &'a dyn CoreBalanceSheet,
    pub advance_fee_oracle: &'a dyn CoreAdvanceFeeOracle,
}

pub fn handle_execute_msg<Msg>(
    ctx: Ctx,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    match msg {
        ExecuteMsg::Admin(admin_msg) => handle_admin_msg(
            ctx.api,
            ctx.admin_repository,
            ctx.vaults,
            ctx.mint,
            info,
            admin_msg,
        ),

        ExecuteMsg::User(user_msg) => handle_user_msg(
            ctx.api,
            ctx.vaults,
            ctx.balance_sheet,
            ctx.advance_fee_oracle,
            info,
            user_msg,
        ),
    }
}

pub fn handle_reply<Msg>(
    storage: &dyn Storage,
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
    advance_fee_oracle: &dyn CoreAdvanceFeeOracle,
    reply: Reply,
) -> Result<(Vec<Cmd>, Response<Msg>), Error> {
    let reason = match reply.id {
        DEPOSIT_REPLY_ID => VaultDepositReason::Deposit,
        REPAY_UNDERLYING_REPLY_ID => VaultDepositReason::RepayUnderlying,
        MINT_REPLY_ID => VaultDepositReason::Mint,
        _ => panic!("unexpected reply"),
    };

    let vault = storage.vault_callback_vault();

    let recipient = storage.vault_callback_recipient();

    let reply_data = parse_reply_execute_data(reply)?
        .data
        .expect("always: a deposit response from the vault");

    let response: VaultDepositResponse = from_json(reply_data)?;

    let cmds = hub(vaults, balance_sheet, advance_fee_oracle).vault_deposit_callback(
        vault.into(),
        recipient.into(),
        reason,
        response.minted_shares.u128(),
        response.deposit_value.u128(),
    )?;

    Ok((cmds, Response::default()))
}

fn vault_metadata(
    storage: &dyn Storage,
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
    vault: VaultId,
) -> Result<VaultMetadata, StdError> {
    if !vaults.is_registered(&vault) {
        return Err(StdError::not_found("vault"));
    }

    let synthetic = vaults.synthetic_asset(&vault);

    let deposit_enabled = vaults.deposits_enabled(&vault);

    let advance_enabled = vaults.advance_enabled(&vault);

    let max_ltv_bps = vaults.max_ltv(&vault).unwrap_or_default().raw();

    let collateral_yield_fee_bps = vaults
        .collateral_yield_fee(&vault)
        .unwrap_or_default()
        .raw();

    let reserve_yield_fee_bps = vaults.reserve_yield_fee(&vault).unwrap_or_default().raw();

    let fixed_advance_fee_bps = vaults.fixed_advance_fee(&vault).unwrap_or_default().raw();

    let advance_fee_recipient = vaults.advance_fee_recipient(&vault).map(Into::into);

    let advance_fee_oracle = vaults.advance_fee_oracle(&vault).map(Into::into);

    let collateral_balance = balance_sheet
        .collateral_balance(&vault)
        .unwrap_or_default()
        .into();

    let collateral_shares = balance_sheet
        .collateral_shares(&vault)
        .unwrap_or_default()
        .into();

    let reserve_balance = balance_sheet
        .reserve_balance(&vault)
        .unwrap_or_default()
        .into();

    let reserve_shares = balance_sheet
        .reserve_shares(&vault)
        .unwrap_or_default()
        .into();

    let treasury_shares = balance_sheet
        .treasury_shares(&vault)
        .unwrap_or_default()
        .into();

    let amo = vaults.amo(&vault).map(Into::into);

    let amo_allocation = vaults.amo_allocation(&vault).unwrap_or_default().raw();

    let amo_shares = balance_sheet.amo_shares(&vault).unwrap_or_default().into();

    let sum_payment_ratio = balance_sheet.overall_sum_payment_ratio(&vault).map(|spr| {
        let timestamp = storage
            .overall_spr_timestamp(&vault)
            .expect("always: present when overall spr is present");

        let raw_spr = spr.into_raw();

        let ratio = Uint256::from_be_bytes(raw_spr.to_be_bytes());

        SumPaymentRatio { ratio, timestamp }
    });

    let deposit_proxy = vaults.deposit_proxy(&vault).map(Identifier::into_string);

    let advance_proxy = vaults.advance_proxy(&vault).map(Identifier::into_string);

    let mint_proxy = vaults.mint_proxy(&vault).map(Identifier::into_string);

    let redeem_proxy = vaults.redeem_proxy(&vault).map(Identifier::into_string);

    Ok(VaultMetadata {
        vault: vault.into_string(),
        synthetic: synthetic.into_string(),
        deposit_enabled,
        advance_enabled,
        max_ltv_bps,
        collateral_yield_fee_bps,
        reserve_yield_fee_bps,
        fixed_advance_fee_bps,
        advance_fee_recipient,
        advance_fee_oracle,
        collateral_balance,
        collateral_shares,
        reserve_balance,
        reserve_shares,
        treasury_shares,
        amo,
        amo_allocation,
        amo_shares,
        sum_payment_ratio,
        deposit_proxy,
        advance_proxy,
        redeem_proxy,
        mint_proxy,
    })
}

fn list_vaults(
    storage: &dyn Storage,
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
) -> Result<Vec<VaultMetadata>, StdError> {
    let mut all_vaults = vec![];

    for i in 0..storage.vault_count() {
        let vault = storage
            .vault_address(i)
            .expect("always: vault address set for i where i < vault count");

        let metadata = vault_metadata(storage, vaults, balance_sheet, vault.into())?;

        all_vaults.push(metadata);
    }

    Ok(all_vaults)
}

fn position(
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
    advance_fee_oracle: &dyn CoreAdvanceFeeOracle,
    vault: VaultId,
    account: Account,
) -> Result<PositionResponse, Error> {
    let hub = hub(vaults, balance_sheet, advance_fee_oracle);

    match hub.evaluate(vault.clone(), account.clone()) {
        Ok(response) => Ok(PositionResponse::from(response.cdp)),

        Err(CoreHubError::SharesValueLoss(_)) => Ok(PositionResponse {
            vault_loss_detected: true,
            // update the stored CDP using stored vault
            ..update_cdp(
                &hub.current_vault_position(&vault),
                hub.current_cdp(&vault, &account),
            )
            .into()
        }),

        Err(err) => Err(err.into()),
    }
}

pub fn handle_query_msg(
    storage: &dyn Storage,
    vaults: &dyn CoreVaults,
    balance_sheet: &dyn CoreBalanceSheet,
    advance_fee_oracle: &dyn CoreAdvanceFeeOracle,
    msg: QueryMsg,
) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::ListVaults {} => list_vaults(storage, vaults, balance_sheet)
            .and_then(|vaults| to_json_binary(&ListVaultsResponse { vaults }))?,

        QueryMsg::VaultMetadata { vault } => {
            vault_metadata(storage, vaults, balance_sheet, vault.into())
                .and_then(|metadata| to_json_binary(&metadata))?
        }

        QueryMsg::Position { account, vault } => position(
            vaults,
            balance_sheet,
            advance_fee_oracle,
            vault.into(),
            account.into(),
        )
        .and_then(|position| to_json_binary(&position).map_err(Error::from))?,

        QueryMsg::Treasury {} => to_json_binary(&TreasuryResponse {
            treasury: balance_sheet.treasury().map(Into::into),
        })?,
    };

    Ok(binary)
}

pub fn handle_hub_cmd<Msg>(
    storage: &mut dyn Storage,
    env: &Env,
    response: &mut Response<Msg>,
    cmd: Cmd,
) -> Result<(), Error> {
    match cmd {
        Cmd::Mint(mint_cmd) => {
            let sub_msg = synthetic_mint::handle_cmd(storage, mint_cmd);

            response.messages.push(sub_msg);
        }

        Cmd::Vault(vault_cmd) => {
            if let Some(sub_msg) = vaults::handle_cmd(storage, vault_cmd) {
                response.messages.push(sub_msg);
            }
        }

        Cmd::BalanceSheet(balance_sheet_cmd) => {
            if let Some(sub_msg) = balance_sheet::handle_cmd(storage, env, balance_sheet_cmd) {
                response.messages.push(sub_msg);
            }
        }
    }

    Ok(())
}

impl From<UserMsg> for ExecuteMsg {
    fn from(v: UserMsg) -> Self {
        Self::User(v)
    }
}

impl From<AdminMsg> for ExecuteMsg {
    fn from(v: AdminMsg) -> Self {
        Self::Admin(v)
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{from_json, to_json_string};

    use super::*;

    #[test]
    fn deserialize_admin_msg() {
        let msg: ExecuteMsg = from_json(
            to_json_string(&AdminMsg::SetTreasury {
                address: "foo".to_owned(),
            })
            .unwrap()
            .as_bytes(),
        )
        .unwrap();

        assert!(matches!(
            msg,
            ExecuteMsg::Admin(AdminMsg::SetTreasury { address }) if address == "foo"
        ));
    }
}
