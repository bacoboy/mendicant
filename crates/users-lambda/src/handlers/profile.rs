use axum::Router;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_profile).patch(patch_profile))
}

async fn get_profile() -> &'static str {
    todo!("return authenticated user profile as Datastar SSE fragment")
}

async fn patch_profile() -> &'static str {
    todo!("update display_name and other self-service fields, return SSE fragment")
}
