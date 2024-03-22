use crate::{Identifier, Sender, UnauthorizedError};

pub type Creator = Identifier;
pub type CurrentAdmin = Identifier;
pub type NextAdmin = Identifier;

/// A zero sized token representing the Admin role - it can only be created by this module.
/// This can be used as a function parameter in other modules, ensuring that the authorization function was called beforehand.
pub struct AdminRole(());

pub enum SetCmd {
    Admin(CurrentAdmin),
    NextAdmin(CurrentAdmin),
}

pub trait Repository {
    /// Returns the creator of the contract (assumed to always be set)
    fn creator(&self) -> Creator;

    /// Returns the current admin, if any
    fn current_admin(&self) -> Option<CurrentAdmin>;

    /// Returns the next admin, if any
    fn next_admin(&self) -> Option<NextAdmin>;
}

pub trait Admin {
    /// Authorize the `sender` as the current admin, obtaining the `AdminRole` token
    fn authorize_admin(&self, sender: Sender) -> Result<AdminRole, UnauthorizedError>;

    /// Transfer the admin role to the next admin - they have to `claim` in order to become the current admin
    fn transfer_admin_role(
        &self,
        sender: Sender,
        next_admin: NextAdmin,
    ) -> Result<SetCmd, UnauthorizedError>;

    /// Claim the current admin role (if the sender matches the next admin)
    fn claim_admin_role(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError>;

    /// Cancel the admin role transfer proces
    fn cancel_next_admin(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError>;
}

pub struct AdminImpl<'a>(&'a dyn Repository);

pub fn admin(repository: &dyn Repository) -> AdminImpl {
    AdminImpl(repository)
}

impl<'a> Admin for AdminImpl<'a> {
    fn authorize_admin(&self, sender: Sender) -> Result<AdminRole, UnauthorizedError> {
        let admin = self.0.current_admin().unwrap_or_else(|| self.0.creator());

        if sender != admin {
            return Err(UnauthorizedError);
        }

        Ok(AdminRole(()))
    }

    fn transfer_admin_role(
        &self,
        sender: Sender,
        next_admin: NextAdmin,
    ) -> Result<SetCmd, UnauthorizedError> {
        self.authorize_admin(sender)?;

        Ok(SetCmd::NextAdmin(next_admin))
    }

    fn claim_admin_role(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError> {
        let next_admin = self.0.next_admin().ok_or(UnauthorizedError)?;

        if sender != next_admin {
            return Err(UnauthorizedError);
        }

        Ok(SetCmd::Admin(next_admin))
    }

    fn cancel_next_admin(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError> {
        self.authorize_admin(sender.clone())?;

        Ok(SetCmd::NextAdmin(sender))
    }
}
