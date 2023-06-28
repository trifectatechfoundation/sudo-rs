use super::Sudoers;

use super::Judgement;
use crate::system::time::Duration;
/// Data types and traits that represent what the "terms and conditions" are after a succesful
/// permission check.
///
/// The trait definitions can be part of some global crate in the future, if we support more
/// than just the sudoers file.
use std::collections::HashSet;
use std::path::Path;

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
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum Authorization {
    Allowed {
        must_authenticate: bool,
        allowed_attempts: u16,
        prior_validity: Duration,
    },
    Forbidden,
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum DirChange<'a> {
    Strict(Option<&'a Path>),
    Any,
}

impl Policy for Judgement {
    fn authorization(&self) -> Authorization {
        if let Some(tag) = &self.flags {
            let allowed_attempts = self.settings.int_value["passwd_tries"].try_into().unwrap();
            let valid_seconds = self.settings.int_value["timestamp_timeout"];
            Authorization::Allowed {
                must_authenticate: tag.passwd,
                allowed_attempts,
                prior_validity: Duration::seconds(valid_seconds),
            }
        } else {
            Authorization::Forbidden
        }
    }

    fn env_keep(&self) -> &HashSet<String> {
        &self.settings.list["env_keep"]
    }

    fn env_check(&self) -> &HashSet<String> {
        &self.settings.list["env_check"]
    }

    fn chdir(&self) -> DirChange {
        match self.flags.as_ref().expect("not authorized").cwd.as_ref() {
            None => DirChange::Strict(None),
            Some(super::ChDir::Any) => DirChange::Any,
            Some(super::ChDir::Path(path)) => DirChange::Strict(Some(path)),
        }
    }

    fn secure_path(&self) -> Option<String> {
        self.settings.str_value["secure_path"]
            .as_ref()
            .map(|s| s.to_string())
    }

    fn use_pty(&self) -> bool {
        self.settings.flags.contains("use_pty")
    }
}

pub trait PreJudgementPolicy {
    fn secure_path(&self) -> Option<String>;
}

impl PreJudgementPolicy for Sudoers {
    fn secure_path(&self) -> Option<String> {
        self.settings.str_value["secure_path"]
            .as_ref()
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sudoers::{ast::Tag, tokens::ChDir};

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
        judge.mod_flag(|tag| tag.passwd = true);
        assert_eq!(
            judge.authorization(),
            Authorization::Allowed {
                must_authenticate: true,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
            }
        );
        judge.mod_flag(|tag| tag.passwd = false);
        assert_eq!(
            judge.authorization(),
            Authorization::Allowed {
                must_authenticate: false,
                allowed_attempts: 3,
                prior_validity: Duration::minutes(15),
            }
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
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/usr")))));
        judge.mod_flag(|tag| tag.cwd = Some(ChDir::Path("/bin".into())));
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/bin")))));
    }
}
