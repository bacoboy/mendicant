use axum::http::StatusCode;
use axum::response::{IntoResponse, Response, Redirect};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match &self {
            Self::Unauthorized => Redirect::to("/login").into_response(),
            Self::Forbidden => (StatusCode::FORBIDDEN, "Forbidden").into_response(),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not Found").into_response(),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()).into_response(),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response(),
        }
    }
}
