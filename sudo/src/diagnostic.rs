use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// TODO: in the future, we will have range information (i.e. error starts here and ends here)

pub(crate) fn cited_error(message: &str, line: usize, col: usize, path: impl AsRef<Path>) {
    let path_str = path.as_ref().display();
    eprintln!("{path_str}:{line}:{col}: {message}");

    let citation = || {
        let inp = BufReader::new(File::open(path).ok()?);
        let line = inp.lines().nth(line - 1)?.ok()?;
        let padding = line
            .chars()
            .take(col - 1)
            .map(|c| if c.is_whitespace() { c } else { ' ' })
            .collect::<String>();
        eprintln!("{line}");
        eprintln!("{padding}^");
        Some(())
    };

    // we ignore any errors in displaying an error
    let _ = citation();
}

macro_rules! diagnostic {
    ($str:expr, $path:tt @ $pos:ident) => {
        if let Some((line, col)) = $pos {
            $crate::diagnostic::cited_error(&format!($str), line, col, $path);
        } else {
            eprintln!($str);
        }
    };
}

pub(crate) use diagnostic;
