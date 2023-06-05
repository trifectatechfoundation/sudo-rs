/// This module contains user-friendly names for the various items in the AST, to report in case they are missing

pub trait UserFriendly {
    const DESCRIPTION: &'static str;
}

// this is in a submodule so it can be switched off and replaced by a blanket implementation for test-cases
#[cfg(not(test))]
mod names {
    use super::*;
    use crate::ast::*;
    use crate::tokens;

    impl UserFriendly for tokens::Digits {
        const DESCRIPTION: &'static str = "number";
    }

    impl UserFriendly for tokens::Numeric {
        const DESCRIPTION: &'static str = "number";
    }

    impl UserFriendly for Identifier {
        const DESCRIPTION: &'static str = "identifier";
    }

    impl<T: UserFriendly> UserFriendly for Vec<T> {
        const DESCRIPTION: &'static str = T::DESCRIPTION;
    }

    impl<T: UserFriendly> UserFriendly for tokens::Meta<T> {
        const DESCRIPTION: &'static str = T::DESCRIPTION;
    }

    impl<T: UserFriendly> UserFriendly for Qualified<T> {
        const DESCRIPTION: &'static str = T::DESCRIPTION;
    }

    impl UserFriendly for tokens::Command {
        const DESCRIPTION: &'static str = "path to binary (or sudoedit)";
    }

    impl UserFriendly
        for (
            SpecList<tokens::Hostname>,
            Vec<(Option<RunAs>, CommandSpec)>,
        )
    {
        const DESCRIPTION: &'static str = tokens::Hostname::DESCRIPTION;
    }

    impl UserFriendly for (Option<RunAs>, CommandSpec) {
        const DESCRIPTION: &'static str = "(users:groups) specification";
    }

    // this can never happen, as parse<Sudo> always succeeds
    impl UserFriendly for Sudo {
        const DESCRIPTION: &'static str = "nothing";
    }

    impl UserFriendly for UserSpecifier {
        const DESCRIPTION: &'static str = "user";
    }

    impl UserFriendly for tokens::Hostname {
        const DESCRIPTION: &'static str = "host name";
    }

    impl UserFriendly for tokens::QuotedText {
        const DESCRIPTION: &'static str = "non-empty string";
    }

    impl UserFriendly for tokens::StringParameter {
        const DESCRIPTION: &'static str = tokens::QuotedText::DESCRIPTION;
    }

    impl UserFriendly for tokens::IncludePath {
        const DESCRIPTION: &'static str = "path to file";
    }

    impl UserFriendly for tokens::AliasName {
        const DESCRIPTION: &'static str = "alias name";
    }

    impl UserFriendly for tokens::EnvVar {
        const DESCRIPTION: &'static str = "environment variable";
    }

    impl UserFriendly for CommandSpec {
        const DESCRIPTION: &'static str = tokens::Command::DESCRIPTION;
    }

    impl UserFriendly for tokens::ChDir {
        const DESCRIPTION: &'static str = "directory or '*'";
    }

    impl UserFriendly for (String, ConfigValue) {
        const DESCRIPTION: &'static str = "parameter";
    }
}

#[cfg(test)]
impl<T: crate::basic_parser::Parse> UserFriendly for T {
    const DESCRIPTION: &'static str = "elem";
}
