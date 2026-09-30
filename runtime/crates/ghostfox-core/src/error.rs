use thiserror::Error;

pub type Result<T> = std::result::Result<T, GhostError>;

#[derive(Debug, Error)]
pub enum GhostError {
    #[error("engine `{0}` not registered or available")]
    EngineUnavailable(String),
    #[error("session `{0}` not found")]
    SessionNotFound(String),
    #[error("page `{0}` not found")]
    PageNotFound(String),
    #[error("fingerprint rejected: {0}")]
    InvalidFingerprint(String),
    #[error("page operation failed: {0}")]
    PageOp(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("engine process terminated unexpectedly")]
    EngineCrashed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
