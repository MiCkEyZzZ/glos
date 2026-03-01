use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ReplayConfiq {
    pub input_path: PathBuf,
    pub target_addr: String,
    pub speed: f64,
    pub loop_playback: bool,
    pub stats_interval_secs: u64,
    pub bind_addr: String,
}

impl ReplayConfiq {
    fn new() -> Self {
        Self {
            input_path: PathBuf::from("recording.glos"),
            target_addr: "127.0.0.1:5555".to_string(),
            speed: 1.0,
            loop_playback: false,
            stats_interval_secs: 5,
            bind_addr: "0.0.0.0:0".to_string(),
        }
    }
}

impl Default for ReplayConfiq {
    fn default() -> Self {
        Self::new()
    }
}
