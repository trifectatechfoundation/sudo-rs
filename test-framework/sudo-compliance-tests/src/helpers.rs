use std::collections::{HashMap, HashSet};

use crate::Result;

pub fn parse_env_output(env_output: &str) -> Result<HashMap<&str, &str>> {
    let mut env = HashMap::new();
    for line in env_output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            env.insert(key, value);
        } else {
            return Err(format!("invalid env syntax: {line}").into());
        }
    }

    Ok(env)
}

pub fn parse_path(path: &str) -> HashSet<&str> {
    path.split(':').collect()
}

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

#[derive(Debug)]
pub struct PsAuxEntry {
    pub command: String,
    pub pid: u32,
    pub process_state: String,
    pub tty: String,
}

impl PsAuxEntry {
    pub fn has_tty(&self) -> bool {
        if self.tty == "?" {
            false
        } else if self.tty.starts_with("pts/") {
            true
        } else {
            unimplemented!()
        }
    }

    pub fn is_session_leader(&self) -> bool {
        self.process_state.contains('s')
    }

    pub fn is_in_the_foreground_process_group(&self) -> bool {
        self.process_state.contains('+')
    }
}
