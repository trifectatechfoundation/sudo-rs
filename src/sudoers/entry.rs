use core::fmt;

use crate::sudoers::{
    ast::{Identifier, Qualified, UserSpecifier},
    tokens::{ChDir, Meta},
};

use self::verbose::Verbose;

use super::{
    ast::{RunAs, Tag},
    tokens::Command,
};

mod verbose;

pub struct Entry<'a> {
    run_as: &'a RunAs,
    cmd_specs: Vec<(Tag, Qualified<&'a Meta<Command>>)>,
}

impl<'a> Entry<'a> {
    pub(super) fn new(
        run_as: &'a RunAs,
        cmd_specs: Vec<(Tag, Qualified<&'a Meta<Command>>)>,
    ) -> Self {
        debug_assert!(!cmd_specs.is_empty());

        Self { run_as, cmd_specs }
    }

    pub fn verbose(self) -> impl fmt::Display + 'a {
        Verbose(self)
    }
}

impl fmt::Display for Entry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { run_as, cmd_specs } = self;

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
        // XXX assumes that the superuser is called "root"
        f.write_str("root")?;
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
    let (cwd, passwd) = if let Some(last_tag) = last_tag {
        let cwd = if last_tag.cwd == tag.cwd {
            None
        } else {
            tag.cwd.as_ref()
        };

        let passwd = if last_tag.passwd == tag.passwd {
            None
        } else {
            tag.passwd
        };

        (cwd, passwd)
    } else {
        (tag.cwd.as_ref(), tag.passwd)
    };

    if let Some(cwd) = cwd {
        f.write_str("CWD=")?;
        match cwd {
            ChDir::Path(path) => write!(f, "{}", path.display())?,
            ChDir::Any => f.write_str("*")?,
        }
        f.write_str(" ")?;
    }

    if let Some(passwd) = passwd {
        let tag = if passwd { "PASSWD" } else { "NOPASSWD" };
        f.write_str(tag)?;
        f.write_str(": ")?;
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
