use core::fmt;

use crate::sudoers::{
    ast::{Identifier, Qualified, UserSpecifier},
    tokens::{ChDir, Meta},
};
use crate::{
    common::{resolve::CurrentUser, SudoString},
    system::{interface::UserId, User},
};

use self::verbose::Verbose;

use super::{
    ast::{Authenticate, RunAs, Tag},
    tokens::Command,
};

mod verbose;

pub struct Entry<'a> {
    run_as: Option<&'a RunAs>,
    cmd_specs: Vec<(Tag, Qualified<&'a Meta<Command>>)>,
}

impl<'a> Entry<'a> {
    pub(super) fn new(
        run_as: Option<&'a RunAs>,
        cmd_specs: Vec<(Tag, Qualified<&'a Meta<Command>>)>,
    ) -> Self {
        debug_assert!(!cmd_specs.is_empty());

        Self { run_as, cmd_specs }
    }

    pub fn verbose(self) -> impl fmt::Display + 'a {
        Verbose(self)
    }
}

static EMPTY_RUNAS: RunAs = RunAs {
    users: Vec::new(),
    groups: Vec::new(),
};

fn singular_runas(name: SudoString) -> RunAs {
    let name = UserSpecifier::User(Identifier::Name(name));
    let name = Qualified::Allow(Meta::Only(name));

    RunAs {
        users: vec![name],
        groups: vec![],
    }
}

impl fmt::Display for Entry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { run_as, cmd_specs } = self;

        let run_as_root = &singular_runas(
            User::from_uid(UserId::ROOT)
                .ok()
                .flatten()
                .map(|u| u.name)
                .unwrap_or(SudoString::new("root".into()).unwrap()),
        );
        let run_as = run_as.unwrap_or(run_as_root);

        f.write_str("    (")?;
        write_users(run_as, f)?;
        if !run_as.groups.is_empty() {
            f.write_str(" : ")?;
        }
        write_groups(run_as, f)?;
        f.write_str(") ")?;

        let mut last_tag = None;
        for (tag, spec) in cmd_specs {
            let is_first_iteration = last_tag.is_none();

            if !is_first_iteration {
                f.write_str(", ")?;
            }

            write_tag(f, tag, last_tag)?;
            last_tag = Some(tag);
            write_spec(f, spec)?;
        }

        Ok(())
    }
}

fn write_users(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    if run_as.users.is_empty() {
        match CurrentUser::resolve() {
            Ok(u) => f.write_str(&u.name)?,
            _ => f.write_str("?")?,
        };
    }

    let mut is_first_user = true;
    for user in &run_as.users {
        if !is_first_user {
            f.write_str(", ")?;
        }
        is_first_user = false;

        let meta = match user {
            Qualified::Allow(meta) => meta,
            Qualified::Forbid(meta) => {
                f.write_str("!")?;
                meta
            }
        };

        match meta {
            Meta::All => f.write_str("ALL")?,
            Meta::Only(user) => {
                let ident = match user {
                    UserSpecifier::User(ident) => ident,
                    UserSpecifier::Group(ident) => {
                        f.write_str("%")?;
                        ident
                    }
                    UserSpecifier::NonunixGroup(ident) => {
                        f.write_str("%:")?;
                        ident
                    }
                };

                match ident {
                    Identifier::Name(name) => f.write_str(name)?,
                    Identifier::ID(id) => write!(f, "#{id}")?,
                }
            }
            Meta::Alias(alias) => f.write_str(alias)?,
        }
    }

    Ok(())
}

fn write_groups(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    let mut is_first_group = true;
    for group in &run_as.groups {
        if !is_first_group {
            f.write_str(", ")?;
        }
        is_first_group = false;

        let meta = match group {
            Qualified::Allow(meta) => meta,
            Qualified::Forbid(meta) => {
                f.write_str("!")?;
                meta
            }
        };

        match meta {
            Meta::All => f.write_str("ALL")?,
            Meta::Only(ident) => match ident {
                Identifier::Name(name) => f.write_str(name)?,
                Identifier::ID(id) => write!(f, "#{id}")?,
            },
            Meta::Alias(alias) => f.write_str(alias)?,
        }
    }

    Ok(())
}

fn write_tag(f: &mut fmt::Formatter, tag: &Tag, last_tag: Option<&Tag>) -> fmt::Result {
    let (cwd, auth) = if let Some(last_tag) = last_tag {
        let cwd = if last_tag.cwd == tag.cwd {
            None
        } else {
            tag.cwd.as_ref()
        };

        let auth = if last_tag.authenticate == tag.authenticate {
            None
        } else {
            Some(tag.authenticate)
        };

        (cwd, auth)
    } else {
        (tag.cwd.as_ref(), Some(tag.authenticate))
    };

    if let Some(cwd) = cwd {
        f.write_str("CWD=")?;
        match cwd {
            ChDir::Path(path) => write!(f, "{}", path.display())?,
            ChDir::Any => f.write_str("*")?,
        }
        f.write_str(" ")?;
    }

    if let Some(auth) = auth {
        if auth != Authenticate::None {
            let tag = if auth == Authenticate::Passwd {
                "PASSWD"
            } else {
                "NOPASSWD"
            };
            f.write_str(tag)?;
            f.write_str(": ")?;
        }
    }

    Ok(())
}

fn write_spec(f: &mut fmt::Formatter, spec: &Qualified<&Meta<Command>>) -> fmt::Result {
    let meta = match spec {
        Qualified::Allow(meta) => meta,
        Qualified::Forbid(meta) => {
            f.write_str("!")?;
            meta
        }
    };

    match meta {
        Meta::All => f.write_str("ALL")?,
        Meta::Only((cmd, args)) => {
            write!(f, "{cmd}")?;
            if let Some(args) = args {
                for arg in args.iter() {
                    write!(f, " {arg}")?;
                }
            }
        }
        Meta::Alias(alias) => f.write_str(alias)?,
    }

    Ok(())
}
