use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const STATE_FILE: &str = "state.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub last_checked_at: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_checked_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

pub fn load() -> io::Result<State> {
    if !Path::new(STATE_FILE).exists() {
        return Ok(State::default());
    }

    let contents = fs::read_to_string(STATE_FILE)?;
    let state = serde_json::from_str(&contents)?;
    Ok(state)
}

pub fn save(state: &State) -> io::Result<()> {
    /*
    let tmp_file = format!("{STATE_FILE}.tmp");
    let json = serde_json::to_string_pretty(state)?;

    {
        let mut file = fs::File::create(&tmp_file)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }

    fs::rename(tmp_file, STATE_FILE)?;
    */
    Ok(())
}