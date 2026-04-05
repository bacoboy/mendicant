use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;

use crate::error::AppError;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/jwks.json", get(jwks))
}

/// Returns the public key set so that any region (or external service) can
/// verify JWTs without calling back to the issuing region.
async fn jwks(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let jwk = state.signer.public_jwk().await?;
    Ok(Json(serde_json::json!({ "keys": [jwk] })))
}
