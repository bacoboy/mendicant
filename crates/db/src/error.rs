use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("item not found")]
    NotFound,

    #[error("condition check failed (concurrent modification)")]
    ConditionalCheckFailed,

    #[error("dynamodb error: {0}")]
    Sdk(String),

    #[error("serialization error: {0}")]
    Serde(String),
}

/// Blanket impl so `?` works on any `SdkError<E>` in repository methods.
impl<E: std::fmt::Debug> From<SdkError<E>> for DbError {
    fn from(e: SdkError<E>) -> Self {
        Self::Sdk(format!("{e:?}"))
    }
}

/// Maps a PutItem error to `ConditionalCheckFailed` when appropriate,
/// otherwise falls back to `Sdk`. Use this instead of `?` on put operations
/// that have a condition expression.
pub fn map_put_error(e: SdkError<PutItemError>) -> DbError {
    if let SdkError::ServiceError(ref se) = e {
        if se.err().is_conditional_check_failed_exception() {
            return DbError::ConditionalCheckFailed;
        }
    }
    DbError::from(e)
}

/// Maps an UpdateItem error to `ConditionalCheckFailed` when appropriate.
pub fn map_update_error(e: SdkError<UpdateItemError>) -> DbError {
    if let SdkError::ServiceError(ref se) = e {
        if se.err().is_conditional_check_failed_exception() {
            return DbError::ConditionalCheckFailed;
        }
    }
    DbError::from(e)
}
