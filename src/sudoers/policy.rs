use super::Sudoers;

use super::Judgement;
use crate::common::{SudoPath, HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1};
use crate::system::time::Duration;
/// Data types and traits that represent what the "terms and conditions" are after a succesful
/// permission check.
///
/// The trait definitions can be part of some global crate in the future, if we support more
/// than just the sudoers file.
use std::collections::HashSet;

pub trait Policy {
    fn authorization(&self) -> Authorization {
        Authorization::Forbidden
    }

    fn chdir(&self) -> DirChange {
        DirChange::Strict(None)
    }

    fn env_keep(&self) -> &HashSet<String>;
    fn env_check(&self) -> &HashSet<String>;

    fn secure_path(&self) -> Option<String>;

    fn use_pty(&self) -> bool;
    fn pwfeedback(&self) -> bool;
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[repr(u32)]
pub enum Authorization {
    Allowed(AuthorizationAllowed) = HARDENED_ENUM_VALUE_0,
    Forbidden = HARDENED_ENUM_VALUE_1,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct AuthorizationAllowed {
    pub must_authenticate: bool,
    pub allowed_attempts: u16,
    pub prior_validity: Duration,
    pub trust_environment: bool,
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[repr(u32)]
pub enum DirChange<'a> {
    Strict(Option<&'a SudoPath>) = HARDENED_ENUM_VALUE_0,
    Any = HARDENED_ENUM_VALUE_1,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[repr(u32)]
pub enum AuthenticatingUser {
    InvokingUser = HARDENED_ENUM_VALUE_0,
    Root = HARDENED_ENUM_VALUE_1,
}

impl Policy for Judgement {
    fn authorization(&self) -> Authorization {
        if let Some(tag) = &self.flags {
            let allowed_attempts = self.settings.passwd_tries().try_into().unwrap();
            let valid_seconds = self.settings.timestamp_timeout();
            Authorization::Allowed(AuthorizationAllowed {
                must_authenticate: tag.needs_passwd(),
                trust_environment: tag.allows_setenv(),
                allowed_attempts,
                prior_validity: Duration::seconds(valid_seconds),
            })
        } else {
            Authorization::Forbidden
        }
    }

    fn env_keep(&self) -> &HashSet<String> {
        self.settings.env_keep()
    }

    fn env_check(&self) -> &HashSet<String> {
        self.settings.env_check()
    }

    fn chdir(&self) -> DirChange {
        match self.flags.as_ref().expect("not authorized").cwd.as_ref() {
            None => DirChange::Strict(None),
            Some(super::ChDir::Any) => DirChange::Any,
            Some(super::ChDir::Path(path)) => DirChange::Strict(Some(path)),
        }
    }

    fn secure_path(&self) -> Option<String> {
        self.settings.secure_path().as_ref().map(|s| s.to_string())
    }

    fn use_pty(&self) -> bool {
        self.settings.use_pty()
    }

    fn pwfeedback(&self) -> bool {
        self.settings.pwfeedback()
    }
}

pub trait PreJudgementPolicy {
    fn secure_path(&self) -> Option<String>;
    fn authenticate_as(&self) -> AuthenticatingUser;
    fn validate_authorization(&self) -> Authorization;
}

impl PreJudgementPolicy for Sudoers {
    fn secure_path(&self) -> Option<String> {
        self.settings.secure_path().as_ref().map(|s| s.to_string())
    }

    fn authenticate_as(&self) -> AuthenticatingUser {
        if self.settings.rootpw() {
            AuthenticatingUser::Root
        } else {
            AuthenticatingUser::InvokingUser
        }
    }

    fn validate_authorization(&self) -> Authorization {
        Authorization::Allowed(AuthorizationAllowed {
            must_authenticate: true,
            trust_environment: false,
            allowed_attempts: self.settings.passwd_tries().try_into().unwrap(),
            prior_validity: Duration::seconds(self.settings.timestamp_timeout()),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sudoers::{
        ast::{Authenticate, Tag},
        tokens::ChDir,
    };

    impl Judgement {
        fn mod_flag(&mut self, mut modify: impl FnMut(&mut Tag)) {
            let mut tag: Tag = self.flags.clone().unwrap_or_default();
            modify(&mut tag);
            self.flags = Some(tag);
        }
    }

    // TODO: refactor the tag-updates to be more readable
    #[test]
    fn authority_xlat_test() {
        let mut judge: Judgement = Default::default();
        assert_eq!(judge.authorization(), Authorization::Forbidden);
        judge.mod_flag(|tag| tag.authenticate = Authenticate::Passwd);
        assert_eq!(
            judge.authorization(),
            Authorization::Allowed(AuthorizationAllowed {
                must_authenticate: true,
                trust_environment: false,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
            })
        );
        judge.mod_flag(|tag| tag.authenticate = Authenticate::Nopasswd);
        assert_eq!(
            judge.authorization(),
            Authorization::Allowed(AuthorizationAllowed {
                must_authenticate: false,
                trust_environment: false,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
            })
        );
    }

    #[test]
    fn chdir_test() {
        let mut judge = Judgement {
            flags: Some(Tag::default()),
            ..Default::default()
        };
        assert_eq!(judge.chdir(), DirChange::Strict(None));
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Any));
        assert_eq!(judge.chdir(), DirChange::Any);
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Path("/usr".into())));
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(&"/usr".into()))));
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Path("/bin".into())));
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(&"/bin".into()))));
    }
}
