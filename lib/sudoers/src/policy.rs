use super::Judgement;
/// Data types and traits that represent what the "terms and conditions" are after a succesful
/// permission check.
///
/// The trait definitions can be part of some global crate in the future, if we support more
/// than just the sudoers file.
use std::collections::HashSet;

pub trait Policy {
    fn env_keep(&self) -> &HashSet<String>;
    fn env_check(&self) -> &HashSet<String>;
    fn authorization(&self) -> Authorization {
        Authorization::Forbidden
    }
}

pub enum Authorization {
    Required,
    Passed,
    Forbidden,
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
        self.settings
            .list
            .get("env_keep")
            .expect("env_keep missing from settings")
    }

    fn env_check(&self) -> &HashSet<String> {
        self.settings
            .list
            .get("env_check")
            .expect("env_check missing from settings")
    }
}
