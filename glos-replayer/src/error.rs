pub type ReplayResult<T> = Result<T, ReplayError>;

#[derive(Debug)]
pub enum ReplayError {
    Io(std::io::Error),
    Glos(glos_types::error::GlosError),
    Config(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            ReplayError::Io(e) => write!(f, "I/O error: {e}"),
            ReplayError::Glos(e) => write!(f, "GLOS error: {e}"),
            ReplayError::Config(s) => write!(f, "Config error: {s}"),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<std::io::Error> for ReplayError {
    fn from(e: std::io::Error) -> Self {
        ReplayError::Io(e)
    }
}

impl From<glos_types::error::GlosError> for ReplayError {
    fn from(e: glos_types::error::GlosError) -> Self {
        ReplayError::Glos(e)
    }
}
