use std::collections::HashMap;

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
