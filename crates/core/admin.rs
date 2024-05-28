use crate::{Identifier, Sender, UnauthorizedError};

pub type Creator = Identifier;
pub type CurrentAdmin = Identifier;
pub type NextAdmin = Identifier;

/// A zero sized token representing the Admin role - it can only be created by this module.
/// This can be used as a function parameter in other modules, ensuring that the authorization function was called beforehand.
#[derive(Debug, Clone, Copy)]
pub struct AdminRole(());

impl AdminRole {
    #[cfg(test)]
    pub(crate) fn mock() -> Self {
        Self(())
    }
}

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

    /// Transfer the admin role to the next admin - they have to `claim_admin_role` in order to become the current admin
    fn transfer_admin_role(
        &self,
        admin_role: AdminRole,
        next_admin: NextAdmin,
    ) -> Result<SetCmd, UnauthorizedError>;

    /// Claim the current admin role (if the sender matches the next admin)
    fn claim_admin_role(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError>;

    /// Cancel the admin role transfer proces
    fn cancel_next_admin(&self, admin_role: AdminRole) -> Result<SetCmd, UnauthorizedError>;
}

pub struct AdminImpl<'a>(&'a dyn Repository);

pub fn admin(repository: &dyn Repository) -> AdminImpl {
    AdminImpl(repository)
}

impl<'a> AdminImpl<'a> {
    fn current_admin(&self) -> CurrentAdmin {
        self.0.current_admin().unwrap_or_else(|| self.0.creator())
    }
}

impl<'a> Admin for AdminImpl<'a> {
    fn authorize_admin(&self, sender: Sender) -> Result<AdminRole, UnauthorizedError> {
        if sender != self.current_admin() {
            return Err(UnauthorizedError);
        }

        Ok(AdminRole(()))
    }

    fn transfer_admin_role(
        &self,
        _: AdminRole,
        next_admin: NextAdmin,
    ) -> Result<SetCmd, UnauthorizedError> {
        Ok(SetCmd::NextAdmin(next_admin))
    }

    fn claim_admin_role(&self, sender: Sender) -> Result<SetCmd, UnauthorizedError> {
        let next_admin = self.0.next_admin().ok_or(UnauthorizedError)?;

        if sender != next_admin {
            return Err(UnauthorizedError);
        }

        Ok(SetCmd::Admin(next_admin))
    }

    fn cancel_next_admin(&self, _: AdminRole) -> Result<SetCmd, UnauthorizedError> {
        let admin = self.current_admin();

        Ok(SetCmd::NextAdmin(admin))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Default)]
    struct Context {
        current_admin: Option<CurrentAdmin>,
        next_admin: Option<NextAdmin>,
    }

    const CREATOR: &str = "creator";
    const ADMIN: &str = "admin";
    const NEXT_ADMIN: &str = "next_admin";
    const RANDO: &str = "some_rando";

    impl Repository for Context {
        fn creator(&self) -> Creator {
            CREATOR.to_owned().into()
        }

        fn current_admin(&self) -> Option<CurrentAdmin> {
            self.current_admin.clone()
        }

        fn next_admin(&self) -> Option<NextAdmin> {
            self.next_admin.clone()
        }
    }

    #[test]
    fn authorize_admin() {
        let creator: Creator = CREATOR.to_owned().into();
        let current_admin: CurrentAdmin = ADMIN.to_owned().into();
        let next_admin: NextAdmin = NEXT_ADMIN.to_owned().into();
        let rando: Identifier = RANDO.to_owned().into();

        let ctx = Context::default();

        assert!(
            admin(&ctx).authorize_admin(creator.clone()).is_ok(),
            "creator is the admin by default"
        );

        let ctx = Context {
            current_admin: Some(current_admin.clone()),
            next_admin: Some(next_admin.clone()),
        };

        assert!(
            admin(&ctx).authorize_admin(current_admin.clone()).is_ok(),
            "current admin can authorize"
        );

        assert!(
            admin(&ctx).authorize_admin(next_admin.clone()).is_err(),
            "next admin is not authorized"
        );

        assert!(
            admin(&ctx).authorize_admin(rando.clone()).is_err(),
            "rando is not authorized"
        );
    }

    #[test]
    fn transfer_admin_role() {
        let next_admin: NextAdmin = NEXT_ADMIN.to_owned().into();

        let cmd = admin(&Context::default())
            .transfer_admin_role(AdminRole::mock(), next_admin.clone())
            .unwrap();

        assert!(matches!(cmd, SetCmd::NextAdmin(a) if a == next_admin))
    }

    #[test]
    fn claim_admin_role() {
        let next_admin: NextAdmin = NEXT_ADMIN.to_owned().into();
        let rando: Identifier = RANDO.to_owned().into();

        let ctx = Context {
            next_admin: Some(next_admin.clone()),
            ..Default::default()
        };

        assert!(
            admin(&ctx).claim_admin_role(rando).is_err(),
            "rando cannot claim admin role"
        );

        let cmd = admin(&ctx).claim_admin_role(next_admin.clone()).unwrap();

        assert!(matches!(cmd, SetCmd::Admin(a) if a == next_admin))
    }

    #[test]
    fn cancel_next_admin() {
        let current_admin: CurrentAdmin = ADMIN.to_owned().into();

        let ctx = Context {
            current_admin: Some(current_admin.clone()),
            ..Default::default()
        };

        let cmd = admin(&ctx).cancel_next_admin(AdminRole::mock()).unwrap();

        assert!(matches!(cmd, SetCmd::NextAdmin(a) if a == current_admin))
    }
}
