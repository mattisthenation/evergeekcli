use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvergeekError {
    #[error("not authenticated — run `evergeek auth login`")]
    NotAuthenticated,
    #[error("authentication token expired — run `evergeek auth login`")]
    TokenExpired,
    #[error("Evernote API error: {0}")]
    Api(String),
    #[error("rate limited by Evernote — retry in {0} seconds")]
    RateLimited(i32),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("config error: {0}")]
    Config(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, EvergeekError>;
