use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,
}
