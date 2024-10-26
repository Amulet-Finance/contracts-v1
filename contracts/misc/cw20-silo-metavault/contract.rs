pub mod cw20;
pub mod msg;
pub mod state;
pub mod strategy;

use anyhow::{bail, Error};
use cosmwasm_std::{
    coins, entry_point, from_json, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, Storage, SubMsg, WasmMsg,
};
use msg::{Cw20Msg, Cw20ReceiveMsg};
use neutron_sdk::bindings::msg::NeutronMsg;

use amulet_core::{mint::Ticker, vault::Cmd as VaultCmd, Decimals};
use amulet_cw::{
    hub::UserMsg as HubMsg,
    mint::TokenFactory as _,
    vault::{
        self, handle_mint_cmd, handle_unbonding_log_cmd, init_mint_msg,
        ExecuteMsg as VaultExecuteMsg, SharesMint, UnbondingLog,
    },
    MigrateMsg,
};
use amulet_ntrn::token_factory::TokenFactory;

use self::msg::{
    ExecuteMsg, InstantiateMsg, MetadataResponse, ProxyExecuteMsg, QueryMsg, StrategyQueryMsg,
};
use self::state::StorageExt as _;
use self::strategy::Strategy;

const PLACEHOLDER_TOKEN_TICKER: &str = "placeholder";

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<NeutronMsg>, Error> {
    deps.api.addr_validate(&msg.owner)?;
    deps.api.addr_validate(&msg.cw20)?;
    deps.api.addr_validate(&msg.hub)?;

    let cw20_decimals = cw20::decimals(deps.as_ref(), &msg.cw20)?;

    if Decimals::from(cw20_decimals) != msg.underlying_decimals {
        bail!(
            "CW20 {} has decimals {cw20_decimals}, expected {}",
            msg.cw20,
            msg.underlying_decimals
        );
    }

    deps.storage.set_owner(&msg.owner);
    deps.storage.set_cw20(&msg.cw20);
    deps.storage.set_hub(&msg.hub);
    deps.storage
        .set_underlying_decimals(msg.underlying_decimals);

    let vault_share_init_msg = init_mint_msg(TokenFactory::new(&env));

    let placeholder_init_msg =
        TokenFactory::new(&env).create(Ticker::new(PLACEHOLDER_TOKEN_TICKER));

    Ok(Response::default()
        .add_message(vault_share_init_msg)
        .add_message(placeholder_init_msg))
}

pub fn execute_vault_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: VaultExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    let strategy = Strategy::new(deps.as_ref(), &env);

    let unbonding_log = UnbondingLog::new(deps.storage);

    let mint = SharesMint::new(deps.storage, &env);

    let (cmds, mut response) =
        vault::handle_execute_msg(&strategy, &unbonding_log, &mint, info, msg)?;

    for cmd in cmds {
        match cmd {
            VaultCmd::Mint(cmd) => {
                let msg = handle_mint_cmd(deps.storage, TokenFactory::new(&env), cmd);

                response.messages.push(SubMsg::new(msg));
            }

            VaultCmd::Strategy(cmd) => {
                if let Some(msg) = strategy::handle_cmd(deps.storage, &env, cmd) {
                    response.messages.push(SubMsg::new(msg));
                }
            }

            VaultCmd::UnbondingLog(cmd) => handle_unbonding_log_cmd(deps.storage, cmd),
        }
    }

    Ok(response)
}

pub fn execute_proxy_msg(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ProxyExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    let msgs = match msg {
        ProxyExecuteMsg::Receive(Cw20ReceiveMsg {
            sender,
            amount,
            msg,
        }) => {
            if amount.is_zero() {
                bail!("received zero amount")
            }

            let owner = deps.storage.owner();

            if sender != owner {
                bail!("unauthorized");
            }

            // only kind of message is Mint, ensure it's valid
            from_json::<Cw20Msg>(msg)?;

            let token_factory = TokenFactory::new(&env);

            let placeholder_denom = token_factory.denom(&Ticker::new(PLACEHOLDER_TOKEN_TICKER));

            let hub_mint_msg = WasmMsg::Execute {
                contract_addr: deps.storage.hub(),
                msg: to_json_binary(&HubMsg::MintOnBehalf {
                    behalf_of: owner,
                    vault: env.contract.address.clone().into_string(),
                })?,
                funds: coins(amount.u128(), &placeholder_denom),
            };

            let mint_placeholder_msg = token_factory.mint(
                placeholder_denom.into(),
                amount.u128(),
                env.contract.address.clone().into_string().into(),
            );

            vec![mint_placeholder_msg, hub_mint_msg.into()]
        }

        ProxyExecuteMsg::Redeem {} => {
            let owner = deps.storage.owner();

            if info.sender.as_str() != owner {
                bail!("unauthorized");
            }

            let hub_redeem_msg = WasmMsg::Execute {
                contract_addr: deps.storage.hub(),
                msg: to_json_binary(&HubMsg::RedeemOnBehalf {
                    behalf_of: owner,
                    vault: env.contract.address.clone().into_string(),
                })?,
                funds: info.funds,
            };

            vec![hub_redeem_msg.into()]
        }
    };

    Ok(Response::default().add_messages(msgs))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<NeutronMsg>, Error> {
    match msg {
        ExecuteMsg::Vault(vault_msg) => execute_vault_msg(deps, env, info, vault_msg),
        ExecuteMsg::Proxy(strategy_msg) => execute_proxy_msg(deps, env, info, strategy_msg),
    }
}

pub fn handle_strategy_query(
    storage: &dyn Storage,
    query: StrategyQueryMsg,
) -> Result<Binary, StdError> {
    match query {
        StrategyQueryMsg::Metadata {} => to_json_binary(&MetadataResponse {
            owner: storage.owner(),
            cw20: storage.cw20(),
            hub: storage.hub(),
            deposits: storage.deposits().into(),
        }),
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, Error> {
    let binary = match msg {
        QueryMsg::Vault(vault_query) => vault::handle_query_msg(
            deps.storage,
            &Strategy::new(deps, &env),
            &UnbondingLog::new(deps.storage),
            &SharesMint::new(deps.storage, &env),
            &env,
            vault_query,
        )?,

        QueryMsg::Strategy(strategy_query) => handle_strategy_query(deps.storage, strategy_query)?,
    };

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, Error> {
    Ok(Response::default())
}

#[cfg(test)]
mod test {
    use amulet_core::vault::MintCmd;
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env},
        to_json_string, Addr, ContractResult, SystemResult, Uint128, WasmQuery,
    };

    use super::*;

    macro_rules! info {
        ($sender:literal) => {
            MessageInfo {
                sender: Addr::unchecked($sender),
                funds: vec![],
            }
        };
        ($sender:literal, $amount:literal, $asset:literal) => {
            MessageInfo {
                sender: Addr::unchecked($sender),
                funds: coins($amount, $asset),
            }
        };
    }

    #[test]
    fn deposit() {
        const OWNER: &str = "owner";
        const CW20: &str = "cw20_token";
        const HUB: &str = "hub_contract";

        let mut deps = mock_dependencies();

        let deposit_amount = 100_000_000_000;

        deps.querier.update_wasm(move |query| {
            let WasmQuery::Smart { msg, contract_addr } = query else {
                panic!("unexpected wasm query: {query:?}");
            };

            let binary = match contract_addr.as_str() {
                CW20 => match from_json(msg).unwrap() {
                    cw20::QueryMsg::TokenInfo {} => to_json_binary(&cw20::TokenInfoResponse {
                        name: "TKN".to_owned(),
                        symbol: "TKN".to_owned(),
                        decimals: 6,
                        total_supply: Uint128::new(deposit_amount),
                    }),
                    cw20::QueryMsg::Balance { .. } => to_json_binary(&cw20::BalanceResponse {
                        balance: Uint128::new(deposit_amount),
                    }),
                },
                _ => panic!("unexpected contract query addr: {contract_addr}"),
            }
            .unwrap();

            SystemResult::Ok(ContractResult::Ok(binary))
        });

        super::instantiate(
            deps.as_mut(),
            mock_env(),
            info!("deployer"),
            InstantiateMsg {
                owner: OWNER.to_owned(),
                cw20: CW20.to_owned(),
                hub: HUB.to_owned(),
                underlying_decimals: 6,
            },
        )
        .unwrap();

        let env = mock_env();

        let strategy = Strategy::new(deps.as_ref(), &env);

        let unbonding_log = UnbondingLog::new(&deps.storage);

        let mint = SharesMint::new(&deps.storage, &env);

        let token_factory = TokenFactory::new(&env);

        let deposit_amount = 100_000_000_000;

        let (cmds, response) = vault::handle_execute_msg::<NeutronMsg>(
            &strategy,
            &unbonding_log,
            &mint,
            MessageInfo {
                sender: Addr::unchecked("owner"),
                funds: coins(
                    deposit_amount,
                    token_factory.denom(&Ticker::new(PLACEHOLDER_TOKEN_TICKER)),
                ),
            },
            VaultExecuteMsg::Deposit {},
        )
        .unwrap();

        for cmd in cmds {
            if let VaultCmd::Mint(MintCmd::Mint { amount, .. }) = cmd {
                assert_eq!(amount.0, deposit_amount * 10u128.pow(12));
            }
        }

        println!("{}", to_json_string(&response.attributes).unwrap());
    }
}
