use core::fmt;

use crate::sudoers::{
    ast::{Authenticate, RunAs, Tag},
    tokens::ChDir,
};

use super::Entry;

pub struct Verbose<'a>(pub Entry<'a>);

impl fmt::Display for Verbose<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(Entry { run_as, cmd_specs }) = self;

        let mut last_tag = None;
        for (tag, cmd_spec) in cmd_specs {
            if last_tag != Some(tag) {
                let is_first_iteration = last_tag.is_none();
                if !is_first_iteration {
                    f.write_str("\n")?;
                }

                write_entry_header(run_as, f)?;
                write_tag(f, tag)?;
                f.write_str("\n    Commands:")?;
            }
            last_tag = Some(tag);

            f.write_str("\n\t")?;
            super::write_spec(f, cmd_spec)?;
        }

        Ok(())
    }
}

fn write_entry_header(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("\nSudoers entry:")?;

    write_users(run_as, f)?;
    write_groups(run_as, f)
}

fn write_users(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("\n    RunAsUsers: ")?;
    super::write_users(run_as, f)
}

fn write_groups(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if run_as.groups.is_empty() {
        return Ok(());
    }

    f.write_str("\n    RunAsGroups: ")?;
    super::write_groups(run_as, f)
}

fn write_tag(f: &mut fmt::Formatter, tag: &Tag) -> fmt::Result {
    if tag.authenticate != Authenticate::None {
        f.write_str("\n    Options: ")?;
        if tag.authenticate != Authenticate::Passwd {
            f.write_str("!")?;
        }
        f.write_str("authenticate")?;
    }

    if let Some(cwd) = &tag.cwd {
        f.write_str("\n    Cwd: ")?;
        match cwd {
            ChDir::Path(path) => write!(f, "{}", path.display())?,
            ChDir::Any => f.write_str("*")?,
        }
    }

    Ok(())
}
