use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

// of course we can also write "file & 0o040 != 0", but this makes the intent explicit
enum Op {
    _Read = 4,
    Write = 2,
    _Exec = 1,
}
enum Category {
    _Owner = 2,
    Group = 1,
    World = 0,
}

fn mode(who: Category, what: Op) -> u32 {
    (what as u32) << (3 * who as u32)
}

pub fn secure_open(path: &Path) -> io::Result<File> {
    let file = File::open(path)?;
    let meta = file.metadata()?;
    let permbits = meta.permissions().mode();
    let error = |msg| Error::new(ErrorKind::PermissionDenied, msg);

    if meta.uid() != 0 {
        Err(error(format!("{} must be owned by root", path.display())))
    } else if meta.gid() != 0 && (permbits & mode(Category::Group, Op::Write) != 0) {
        Err(error(format!(
            "{} cannot be group-writable",
            path.display()
        )))
    } else if permbits & mode(Category::World, Op::Write) != 0 {
        Err(error(format!(
            "{} cannot be world-writable",
            path.display()
        )))
    } else {
        Ok(file)
    }
}

#[cfg(test)]
mod test {
    use super::secure_open;
    use std::path::Path;

    #[test]
    fn secure_open_is_predictable() {
        // /etc/hosts should be readable and "secure" (if this test fails, you have been compromised)
        assert!(std::fs::File::open("/etc/hosts").is_ok());
        assert!(secure_open(Path::new("/etc/hosts")).is_ok());
        // /var/log/utmp should be readable, but not secure (writeable by group other than root)
        assert!(std::fs::File::open("/var/log/wtmp").is_ok());
        assert!(secure_open(Path::new("/var/log/wtmp")).is_err());
        // /etc/shadow should not be readable
        assert!(std::fs::File::open("/etc/shadow").is_err());
        assert!(secure_open(Path::new("/etc/shadow")).is_err());
    }
}
