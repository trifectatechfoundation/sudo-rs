use super::Judgement;
/// Data types and traits that represent what the "terms and conditions" are after a succesful
/// permission check.
use std::collections::HashSet;

pub trait Configuration {
    fn env_keep(&self) -> &HashSet<String>;
    fn env_check(&self) -> &HashSet<String>;
}

pub enum Authorization {
    Required,
    Passed,
    Forbidden,
}

impl Judgement {
    pub fn authorization(&self) -> Authorization {
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
}

impl Configuration for Judgement {
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
