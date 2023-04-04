use crate::Sudoers;

use super::Judgement;
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
}

#[must_use]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum Authorization {
    Required,
    Passed,
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
            if !tag.passwd {
                Authorization::Passed
            } else {
                Authorization::Required
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
}

pub trait PreJudgementPolicy {
    fn secure_path(&self) -> Option<&str>;
}

impl PreJudgementPolicy for Sudoers {
    fn secure_path(&self) -> Option<&str> {
        self.settings.str_value["secure_path"].as_deref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Tag;

    // TODO: refactor the tag-updates to be more readable
    #[test]
    fn authority_xlat_test() {
        let mut judge: Judgement = Default::default();
        assert_eq!(judge.authorization(), Authorization::Forbidden);
        judge.flags = Some(Tag {
            passwd: true,
            ..judge.flags.unwrap_or_default()
        });
        assert_eq!(judge.authorization(), Authorization::Required);
        judge.flags = Some(Tag {
            passwd: false,
            ..judge.flags.unwrap_or_default()
        });
        assert_eq!(judge.authorization(), Authorization::Passed);
    }

    #[test]
    fn chdir_test() {
        let mut judge = Judgement {
            flags: Some(Tag::default()),
            ..Default::default()
        };
        assert_eq!(judge.chdir(), DirChange::Strict(None));
        judge.flags = Some(Tag {
            cwd: Some(crate::ChDir::Any),
            ..judge.flags.unwrap_or_default()
        });
        assert_eq!(judge.chdir(), DirChange::Any);
        judge.flags = Some(Tag {
            cwd: Some(crate::ChDir::Path("/usr".into())),
            ..judge.flags.unwrap_or_default()
        });
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/usr")))));
        judge.flags = Some(Tag {
            cwd: Some(crate::ChDir::Path("/bin".into())),
            ..judge.flags.unwrap_or_default()
        });
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/bin")))));
    }
}
