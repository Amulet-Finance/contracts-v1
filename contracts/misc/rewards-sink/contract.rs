use cosmwasm_schema::cw_serde;
use cosmwasm_std::{entry_point, BankMsg, DepsMut, Env, MessageInfo, Response, StdError, Storage};

use amulet_cw::StorageExt as _;

#[rustfmt::skip]
mod key {
    macro_rules! key {
        ($k:literal) => {
            concat!("rewards_sink::", $k)
        };
    }

    pub const OWNER         : &str = key!("owner");
    pub const REWARDS_DENOM : &str = key!("rewards_denom");
}

trait StorageExt: Storage {
    fn set_owner(&mut self, address: &str) {
        self.set_string(key::OWNER, address)
    }

    fn owner(&self) -> String {
        self.string_at(key::OWNER)
            .expect("always: set during initialisation")
    }

    fn set_rewards_denom(&mut self, denom: &str) {
        self.set_string(key::REWARDS_DENOM, denom)
    }

    fn rewards_denom(&self) -> String {
        self.string_at(key::REWARDS_DENOM)
            .expect("always: set during initialisation")
    }
}

impl<T> StorageExt for T where T: Storage + ?Sized {}

#[cw_serde]
pub struct InstantiateMsg {
    rewards_denom: String,
}

#[cw_serde]
pub struct CollectRewards {}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> {
    deps.storage.set_owner(info.sender.as_str());
    deps.storage.set_rewards_denom(&msg.rewards_denom);

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _msg: CollectRewards,
) -> Result<Response, StdError> {
    if info.sender != deps.storage.owner() {
        return Err(StdError::generic_err("unauthorized"));
    }

    let rewards_balance = deps
        .querier
        .query_balance(env.contract.address, deps.storage.rewards_denom())?;

    Ok(Response::default().add_message(BankMsg::Send {
        to_address: info.sender.into_string(),
        amount: vec![rewards_balance],
    }))
}
