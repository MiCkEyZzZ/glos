use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub input_path: PathBuf,
    pub target_addr: SocketAddr,
    pub speed: f64,
    pub loop_playback: bool,
    pub stats_interval_secs: u64,
    pub bind_addr: SocketAddr,
}

impl ReplayConfig {
    fn new() -> Self {
        Self {
            input_path: PathBuf::from("recording.glos"),
            target_addr: "127.0.0.1:5555".parse().unwrap(),
            speed: 1.0,
            loop_playback: false,
            stats_interval_secs: 5,
            bind_addr: "0.0.0.0:0".parse().unwrap(),
        }
    }
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self::new()
    }
}
