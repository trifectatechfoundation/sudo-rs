//! Test helpers

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
        if self.tty == "?" {
            false
        } else if self.tty.starts_with("pts/") {
            true
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
