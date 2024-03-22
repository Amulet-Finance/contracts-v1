use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Api, Binary, MessageInfo, StdError};

use amulet_core::{
    admin::{admin, Admin, AdminRole, Repository as CoreRepository, SetCmd},
    Identifier, UnauthorizedError,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),
    #[error(transparent)]
    CosmWasm(#[from] StdError),
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Transfer admin role to a new address
    TransferAdminRole { next_admin: String },

    /// Complete admin role transferral
    ClaimAdminRole {},

    /// Cancel admin role transferral
    CancelRoleTransfer {},
}

#[cw_serde]
pub struct CurrentAdminResponse {
    /// The current admin, if None the contract creator is the admin
    current_admin: Option<String>,
}

#[cw_serde]
pub struct PendingAdminResponse {
    /// The pending admin, they will become admin if they claim the role
    pending_admin: Option<String>,
}

#[cw_serde]
#[derive(cosmwasm_schema::QueryResponses)]
pub enum QueryMsg {
    /// Returns the current admin role holder
    #[returns(CurrentAdminResponse)]
    CurrentAdmin {},

    /// Returns the pending admin role holder
    #[returns(PendingAdminResponse)]
    PendingAdmin {},
}

pub fn handle_execute_msg(
    api: &dyn Api,
    repository: &dyn CoreRepository,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<SetCmd, Error> {
    let admin = admin(repository);

    let cmd = match msg {
        ExecuteMsg::TransferAdminRole { next_admin } => {
            api.addr_validate(&next_admin)?;

            admin.transfer_admin_role(info.sender.into_string().into(), next_admin.into())?
        }

        ExecuteMsg::ClaimAdminRole {} => {
            admin.claim_admin_role(info.sender.into_string().into())?
        }

        ExecuteMsg::CancelRoleTransfer {} => {
            admin.cancel_next_admin(info.sender.into_string().into())?
        }
    };

    Ok(cmd)
}

pub fn handle_query_msg(
    repository: &dyn CoreRepository,
    msg: QueryMsg,
) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::CurrentAdmin {} => {
            let current_admin = repository.current_admin().map(Identifier::into_string);

            to_json_binary(&CurrentAdminResponse { current_admin })
        }

        QueryMsg::PendingAdmin {} => {
            let pending_admin = repository.next_admin().map(Identifier::into_string);

            to_json_binary(&PendingAdminResponse { pending_admin })
        }
    }
}

pub fn get_admin_role(
    repository: &dyn CoreRepository,
    info: MessageInfo,
) -> Result<AdminRole, Error> {
    admin(repository)
        .authorize_admin(info.sender.into_string().into())
        .map_err(Error::from)
}

pub mod repository {
    use cosmwasm_std::{MessageInfo, Storage};

    use amulet_core::{
        admin::{Creator, CurrentAdmin, NextAdmin, Repository as CoreRepository, SetCmd},
        Identifier,
    };

    use crate::StorageExt as _;

    pub struct Repository<'a>(&'a dyn Storage);

    impl<'a> Repository<'a> {
        pub fn new(storage: &'a dyn Storage) -> Self {
            Self(storage)
        }
    }

    #[rustfmt::skip]
    mod key {
        macro_rules! key {
            ($k:literal) => {
                concat!("admin_repository::", $k)
            };
        }

        pub const CREATOR       : &str = key!("creator");
        pub const CURRENT_ADMIN : &str = key!("current_admin");
        pub const NEXT_ADMIN    : &str = key!("next_admin");
    }

    impl<'a> CoreRepository for Repository<'a> {
        fn creator(&self) -> Creator {
            self.0
                .string_at(key::CREATOR)
                .map(Identifier::from)
                .expect("creator set during initialization")
        }

        fn current_admin(&self) -> Option<CurrentAdmin> {
            self.0.string_at(key::CURRENT_ADMIN).map(Identifier::from)
        }

        fn next_admin(&self) -> Option<NextAdmin> {
            self.0.string_at(key::NEXT_ADMIN).map(Identifier::from)
        }
    }

    pub fn init(storage: &mut dyn Storage, info: &MessageInfo) {
        storage.set_string(key::CREATOR, info.sender.as_str())
    }

    pub fn handle_cmd(storage: &mut dyn Storage, cmd: SetCmd) {
        match cmd {
            SetCmd::Admin(admin) => {
                storage.set_string(key::CURRENT_ADMIN, &admin);
            }

            SetCmd::NextAdmin(next_admin) => {
                storage.set_string(key::NEXT_ADMIN, &next_admin);
            }
        }
    }
}

pub use repository::{handle_cmd, init, Repository};
