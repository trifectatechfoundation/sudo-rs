use core::fmt;

use crate::gettext::xlat;
use crate::sudoers::{
    ast::{Authenticate, RunAs, Tag},
    tokens::ChDir,
};

use super::Entry;

pub struct Verbose<'a>(pub Entry<'a>);

impl fmt::Display for Verbose<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(Entry {
            run_as,
            cmd_specs,
            cmd_alias,
        }) = self;

        let root_runas = super::root_runas();
        let run_as = run_as.unwrap_or(&root_runas);

        let mut last_tag = None;
        for (tag, cmd_spec) in cmd_specs {
            if last_tag != Some(tag) {
                let is_first_iteration = last_tag.is_none();
                if !is_first_iteration {
                    f.write_str("\n")?;
                }

                write_entry_header(run_as, f)?;
                write_tag(f, tag)?;
                write!(f, "\n    {}", xlat!("Commands:"))?;
            }
            last_tag = Some(tag);

            f.write_str("\n\t")?;
            super::write_spec(f, cmd_spec, cmd_alias.iter().rev(), true, "\n\t")?;
        }

        Ok(())
    }
}

fn write_entry_header(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "\n{}", xlat!("Sudoers entry:"))?;

    write_users(run_as, f)?;
    write_groups(run_as, f)
}

fn write_users(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // TRANSLATORS: This is sudo-specific jargon.
    write!(f, "\n    {}: ", xlat!("RunAsUsers"))?;
    super::write_users(run_as, f)
}

fn write_groups(run_as: &RunAs, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if run_as.groups.is_empty() {
        return Ok(());
    }

    // TRANSLATORS: This is sudo-specific jargon.
    write!(f, "\n    {}: ", xlat!("RunAsGroups"))?;
    super::write_groups(run_as, f)
}

fn write_tag(f: &mut fmt::Formatter, tag: &Tag) -> fmt::Result {
    if tag.authenticate != Authenticate::None {
        write!(f, "\n    {}: ", xlat!("Options"))?;
        if tag.authenticate != Authenticate::Passwd {
            f.write_str("!")?;
        }
        f.write_str("authenticate")?;
    }

    if let Some(cwd) = &tag.cwd {
        // TRANSLATORS: This is sudo-specific jargon.
        write!(f, "\n    {}: ", xlat!("Cwd"))?;
        match cwd {
            ChDir::Path(path) => write!(f, "{}", path.display())?,
            ChDir::Any => f.write_str("*")?,
        }
    }

    Ok(())
}
