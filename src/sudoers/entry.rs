use core::fmt;

use crate::sudoers::{
    VecOrd,
    ast::{Identifier, Qualified, UserSpecifier},
    tokens::{ChDir, Meta},
};
use crate::{
    common::{DisplayOsStr, SudoString, resolve::CurrentUser},
    system::{User, interface::UserId},
};

use self::verbose::Verbose;

use super::{
    ast::{Authenticate, Def, EnvironmentControl, ExecControl, RunAs, Tag},
    tokens::{Args, Command},
};

mod verbose;

pub struct Entry<'a> {
    run_as: Option<&'a RunAs>,
    cmd_specs: Vec<(Tag, &'a Qualified<Meta<Command>>)>,
    cmd_alias: &'a VecOrd<Def<Command>>,
}

impl<'a> Entry<'a> {
    pub(super) fn new(
        run_as: Option<&'a RunAs>,
        cmd_specs: Vec<(Tag, &'a Qualified<Meta<Command>>)>,
        cmd_alias: &'a VecOrd<Def<Command>>,
    ) -> Self {
        debug_assert!(!cmd_specs.is_empty());

        Self {
            run_as,
            cmd_specs,
            cmd_alias,
        }
    }

    pub fn verbose(self) -> impl fmt::Display + 'a {
        Verbose(self)
    }
}

fn root_runas() -> RunAs {
    let name = User::from_uid(UserId::ROOT)
        .ok()
        .flatten()
        .map(|u| u.name)
        .unwrap_or(SudoString::new("root".into()).unwrap());

    let name = UserSpecifier::User(Identifier::Name(name));
    let name = Qualified::Allow(Meta::Only(name));

    RunAs {
        users: vec![name],
        groups: vec![],
    }
}

impl fmt::Display for Entry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            run_as,
            cmd_specs,
            cmd_alias,
        } = self;

        let root_runas = root_runas();
        let run_as = run_as.unwrap_or(&root_runas);

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

            write_tag(f, tag, &mut last_tag, spec)?;

            // cmd_alias is to be topologically sorted (dependencies come before dependents),
            // the argument to write_spec needs to have dependents before dependencies.
            write_spec(f, spec, cmd_alias.iter().rev(), true, ", ")?;
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

fn write_tag(
    f: &mut fmt::Formatter,
    tag: &Tag,
    last_tag: &mut Option<Tag>,
    spec: &Qualified<Meta<Command>>,
) -> fmt::Result {
    let last_tag = last_tag.get_or_insert(Tag::default());

    if tag.apparmor_profile != last_tag.apparmor_profile {
        f.write_str("APPARMOR_PROFILE=")?;
        let profile = tag
            .apparmor_profile
            .as_ref()
            .expect("sudoers spec turned off");
        f.write_str(profile)?;
        f.write_str(" ")?;
    }

    if tag.cwd != last_tag.cwd {
        f.write_str("CWD=")?;
        match tag.cwd.as_ref().expect("sudoers spec turned off") {
            ChDir::Path(path) => write!(f, "{}", path.display())?,
            ChDir::Any => f.write_str("*")?,
        }
        f.write_str(" ")?;
    }

    let mut write_tag = |text, status: bool| {
        if !status {
            f.write_str("NO")?;
        };
        f.write_str(text)?;
        f.write_str(": ")
    };

    if tag.env != last_tag.env
        && !(matches!(spec, Qualified::Allow(Meta::All)) && tag.env == EnvironmentControl::Setenv)
    {
        write_tag("SETENV", tag.env == EnvironmentControl::Setenv)?;
    }
    if tag.noexec != last_tag.noexec {
        write_tag("EXEC", tag.noexec == ExecControl::Exec)?;
    }
    if tag.authenticate != last_tag.authenticate {
        write_tag("PASSWD", tag.authenticate != Authenticate::Nopasswd)?;
    }

    *last_tag = tag.clone();
    Ok(())
}

fn write_spec<'a>(
    f: &mut fmt::Formatter,
    spec: &Qualified<Meta<Command>>,
    mut alias_list: impl Iterator<Item = &'a Def<Command>> + Clone,
    mut sign: bool,
    separator: &str,
) -> fmt::Result {
    let meta = match spec {
        Qualified::Allow(meta) => meta,
        Qualified::Forbid(meta) => {
            sign = !sign;
            meta
        }
    };

    match meta {
        Meta::All | Meta::Only(_) if !sign => f.write_str("!")?,
        _ => {}
    }

    match meta {
        Meta::All => f.write_str("ALL")?,

        Meta::Only((cmd, args)) => {
            write!(f, "{cmd}")?;
            match args {
                Args::Exact(args) => {
                    for arg in args {
                        write!(f, " {}", DisplayOsStr(arg))?;
                    }
                    if args.is_empty() {
                        write!(f, " \"\"")?;
                    }
                }
                Args::Prefix(args) => {
                    for arg in args {
                        write!(f, " {}", DisplayOsStr(arg))?;
                    }
                    if !args.is_empty() {
                        write!(f, " *")?;
                    }
                }
            }
        }
        Meta::Alias(alias) => {
            if let Some(Def(_, spec_list)) = alias_list.find(|Def(id, _)| id == alias) {
                let mut is_first_iteration = true;
                for spec in spec_list {
                    if !is_first_iteration {
                        f.write_str(separator)?;
                    }
                    // 1) this recursion will terminate, since "alias_list" has become smaller
                    //    by the "alias_list.find()" above
                    // 2) to get the correct macro expansion, alias_list has to be (reverse-)topologically
                    //    sorted so that "later" definitions do not refer back to "earlier" definitions.
                    write_spec(f, spec, alias_list.clone(), sign, separator)?;
                    is_first_iteration = false;
                }
            } else {
                f.write_str("???")?
            }
        }
    }

    Ok(())
}
