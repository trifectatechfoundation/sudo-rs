use super::Sudoers;

use super::Judgement;
use crate::common::{
    SudoPath, HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2,
};
use crate::sudoers::ast::{Noexec, Tag};
use crate::system::{time::Duration, Hostname, User};
/// Data types and traits that represent what the "terms and conditions" are after a succesful
/// permission check.
///
/// The trait definitions can be part of some global crate in the future, if we support more
/// than just the sudoers file.
use std::collections::HashSet;

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[repr(u32)]
pub enum Authorization<T = ()> {
    Allowed(Authentication, T) = HARDENED_ENUM_VALUE_0,
    Forbidden = HARDENED_ENUM_VALUE_1,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[must_use]
pub struct Authentication {
    pub must_authenticate: bool,
    pub credential: AuthenticatingUser,
    pub allowed_attempts: u16,
    pub prior_validity: Duration,
    pub pwfeedback: bool,
}

impl super::Settings {
    pub(super) fn to_auth(&self, tag: &Tag) -> Authentication {
        Authentication {
            must_authenticate: tag.needs_passwd(),
            allowed_attempts: self.passwd_tries().try_into().unwrap(),
            prior_validity: Duration::seconds(self.timestamp_timeout()),
            pwfeedback: self.pwfeedback(),
            credential: if self.rootpw() {
                AuthenticatingUser::Root
            } else if self.targetpw() {
                AuthenticatingUser::TargetUser
            } else {
                AuthenticatingUser::InvokingUser
            },
        }
    }
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Restrictions<'a> {
    pub use_pty: bool,
    pub trust_environment: bool,
    pub noexec: bool,
    pub env_keep: &'a HashSet<String>,
    pub env_check: &'a HashSet<String>,
    pub chdir: DirChange<'a>,
    pub path: Option<&'a str>,
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
    TargetUser = HARDENED_ENUM_VALUE_2,
}

impl Judgement {
    pub fn authorization(&self) -> Authorization<Restrictions> {
        if let Some(tag) = &self.flags {
            Authorization::Allowed(
                self.settings.to_auth(tag),
                Restrictions {
                    use_pty: self.settings.use_pty(),
                    trust_environment: tag.allows_setenv(),
                    noexec: match tag.noexec {
                        Noexec::Implicit => self.settings.noexec(),
                        Noexec::Exec => false,
                        Noexec::Noexec => true,
                    },
                    env_keep: self.settings.env_keep(),
                    env_check: self.settings.env_check(),
                    chdir: match tag.cwd.as_ref() {
                        None => DirChange::Strict(None),
                        Some(super::ChDir::Any) => DirChange::Any,
                        Some(super::ChDir::Path(path)) => DirChange::Strict(Some(path)),
                    },
                    path: self.settings.secure_path(),
                },
            )
        } else {
            Authorization::Forbidden
        }
    }
}

impl Sudoers {
    pub fn search_path(
        &mut self,
        on_host: &Hostname,
        current_user: &User,
        target_user: &User,
    ) -> Option<&str> {
        self.specify_host_user_runas(on_host, current_user, target_user);
        self.settings.secure_path()
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

    #[test]
    fn authority_xlat_test() {
        let mut judge: Judgement = Default::default();
        assert_eq!(judge.authorization(), Authorization::Forbidden);
        judge.mod_flag(|tag| tag.authenticate = Authenticate::Passwd);
        let Authorization::Allowed(auth, restrictions) = judge.authorization() else {
            panic!();
        };
        assert_eq!(
            auth,
            Authentication {
                must_authenticate: true,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
                credential: AuthenticatingUser::InvokingUser,
                pwfeedback: false,
            },
        );

        let mut judge = judge.clone();
        judge.mod_flag(|tag| tag.authenticate = Authenticate::Nopasswd);
        let Authorization::Allowed(auth, restrictions2) = judge.authorization() else {
            panic!();
        };
        assert_eq!(
            auth,
            Authentication {
                must_authenticate: false,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
                credential: AuthenticatingUser::InvokingUser,
                pwfeedback: false,
            },
        );
        assert_eq!(restrictions, restrictions2);
    }

    #[test]
    fn chdir_test() {
        let mut judge = Judgement {
            flags: Some(Tag::default()),
            ..Default::default()
        };
        fn chdir(judge: &mut Judgement) -> DirChange {
            let Authorization::Allowed(_, ctl) = judge.authorization() else {
                panic!()
            };
            ctl.chdir
        }
        assert_eq!(chdir(&mut judge), DirChange::Strict(None));
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Any));
        assert_eq!(chdir(&mut judge), DirChange::Any);
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Path("/usr".into())));
        assert_eq!(chdir(&mut judge), (DirChange::Strict(Some(&"/usr".into()))));
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Path("/bin".into())));
        assert_eq!(chdir(&mut judge), (DirChange::Strict(Some(&"/bin".into()))));
    }
}
