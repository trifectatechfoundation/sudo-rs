use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::Path;

pub(crate) fn cited_error(message: &str, range: Range<(usize, usize)>, path: impl AsRef<Path>) {
    let path_str = path.as_ref().display();
    let Range {
        start: (line, col),
        end: (end_line, mut end_col),
    } = range;
    eprintln_ignore_io_error!("{path_str}:{line}:{col}: {message}");

    // we won't try to "span" errors across multiple lines
    if line != end_line {
        end_col = col;
    }

    let citation = || {
        let inp = BufReader::new(File::open(path).ok()?);
        let line = inp.lines().nth(line - 1)?.ok()?;
        let padding = line
            .chars()
            .take(col - 1)
            .map(|c| if c.is_whitespace() { c } else { ' ' })
            .collect::<String>();
        let lineunder = std::iter::repeat('~')
            .take(end_col - col)
            .skip(1)
            .collect::<String>();
        eprintln_ignore_io_error!("{line}");
        eprintln_ignore_io_error!("{padding}^{lineunder}");
        Some(())
    };

    // we ignore any errors in displaying an error
    let _ = citation();
}

macro_rules! diagnostic {
    ($str:expr, $path:tt @ $pos:ident) => {
        if let Some(range) = $pos {
            $crate::sudo::diagnostic::cited_error(&format!($str), range, $path);
        } else {
            eprintln_ignore_io_error!("sudo-rs: {}", format!($str));
        }
    };
    ($str:expr) => {{
        eprintln_ignore_io_error!("sudo-rs: {}", format!($str));
    }};
}

pub(crate) use diagnostic;
