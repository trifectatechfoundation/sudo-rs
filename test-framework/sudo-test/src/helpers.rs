//! Test helpers

/// A command which will print the owner of it's TTY in the format "username group"
#[cfg(target_os = "linux")]
pub const PRINT_PTY_OWNER: &str = "stat $(tty) --format '%U %G'";

/// A command which will print the owner of it's TTY in the format "username group"
#[cfg(target_os = "freebsd")]
pub const PRINT_PTY_OWNER: &str = "stat -f '%Su %Sg' $(tty)";

/// Check that the ls output matches the expectation while being insensitive to the exact output of
/// the system ls version.
#[track_caller]
pub fn assert_ls_output(ls_output: &str, mode: &str, user: &str, group: &str) {
    let parts = ls_output
        .split(' ')
        .filter(|part| !part.is_empty()) // FreeBSD ls often uses multiple spaces as seperator
        .collect::<Vec<_>>();

    assert_eq!(parts[0], mode);
    assert_eq!(parts[2], user);
    assert_eq!(parts[3], group);
}

/// parse the output of `ps aux`
pub fn parse_ps_aux(ps_aux: &str) -> Vec<PsAuxEntry> {
    let mut entries = vec![];
    for line in ps_aux.lines().skip(1 /* header */) {
        let columns = line.split_ascii_whitespace().collect::<Vec<_>>();

        let entry = PsAuxEntry {
            command: columns[10..].join(" "),
            pid: columns[1].parse().expect("invalid PID"),
            process_state: columns[7].to_owned(),
            tty: columns[6].to_owned(),
        };

        entries.push(entry);
    }

    entries
}

/// an entry / row in `ps aux` output
#[derive(Debug)]
pub struct PsAuxEntry {
    /// command column
    pub command: String,
    /// pid column
    pub pid: u32,
    /// process state column
    pub process_state: String,
    /// tty column
    pub tty: String,
}

impl PsAuxEntry {
    /// whether the process has an associated PTY
    pub fn has_tty(&self) -> bool {
        if cfg!(target_os = "linux") {
            // On Linux the PTY is either ? when there is no pty, starts with pts/ for a pseudo
            // terminal or starts with tty in case of a virtual terminal. The last case shouldn't
            // happen inside of containers.
            if self.tty == "?" {
                false
            } else if self.tty.starts_with("pts/") {
                true
            } else {
                unimplemented!()
            }
        } else if cfg!(target_os = "freebsd") {
            // On FreeBSD the PTY is either ? or - when there is no pty, or is an integer
            // potentially prefixed by v.
            if self.tty == "?" || self.tty == "-" {
                false
            } else {
                true
            }
        } else {
            unimplemented!()
        }
    }

    /// whethe the process is a session leader
    pub fn is_session_leader(&self) -> bool {
        self.process_state.contains('s')
    }

    /// whethe the process is in the foreground process group
    pub fn is_in_the_foreground_process_group(&self) -> bool {
        self.process_state.contains('+')
    }
}
