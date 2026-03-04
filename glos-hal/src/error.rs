use std::fmt;

#[derive(Debug)]
pub enum HalError {
    InitialFailed,
    StreamError,
    DeviceDisconected,
    Unsupported,
    Other(String),
}

impl fmt::Display for HalError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            HalError::InitialFailed => write!(f, "Initialization failed"),
            HalError::StreamError => write!(f, "Streaming error"),
            HalError::DeviceDisconected => write!(f, "Device disconected"),
            HalError::Unsupported => write!(f, "Unsupported operation"),
            HalError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for HalError {}
