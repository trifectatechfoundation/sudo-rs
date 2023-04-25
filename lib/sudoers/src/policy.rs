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

    fn mail_badpass(&self) -> bool;

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

    fn mail_badpass(&self) -> bool {
        self.settings.flags.contains("mail_badpass")
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
    use crate::Tag;

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
        assert_eq!(judge.authorization(), Authorization::Required);
        judge.mod_flag(|tag| tag.passwd = false);
        assert_eq!(judge.authorization(), Authorization::Passed);
    }

    #[test]
    fn chdir_test() {
        let mut judge = Judgement {
            flags: Some(Tag::default()),
            ..Default::default()
        };
        assert_eq!(judge.chdir(), DirChange::Strict(None));
        judge.mod_flag(|tag| tag.cwd = Some(crate::ChDir::Any));
        assert_eq!(judge.chdir(), DirChange::Any);
        judge.mod_flag(|tag| tag.cwd = Some(crate::ChDir::Path("/usr".into())));
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/usr")))));
        judge.mod_flag(|tag| tag.cwd = Some(crate::ChDir::Path("/bin".into())));
        assert_eq!(judge.chdir(), (DirChange::Strict(Some(Path::new("/bin")))));
    }
}
