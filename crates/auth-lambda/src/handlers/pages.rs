use axum::Router;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(login_page))
        .route("/register", get(register_page))
        .route("/activate", get(activate_page))
}

async fn login_page() -> &'static str {
    todo!("render login page HTML")
}

async fn register_page() -> &'static str {
    todo!("render register page HTML")
}

async fn activate_page() -> &'static str {
    todo!("render OAuth device activation page HTML")
}
