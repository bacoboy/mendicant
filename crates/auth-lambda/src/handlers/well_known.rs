use axum::Router;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/jwks.json", get(jwks))
}

/// Returns the public key set so that any region (or external service) can
/// verify JWTs without calling back to the issuing region.
async fn jwks() -> &'static str {
    todo!("return JWKS JSON from Signer::public_jwk()")
}
