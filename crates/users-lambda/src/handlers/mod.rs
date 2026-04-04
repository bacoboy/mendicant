use axum::Router;

use crate::state::AppState;

mod admin;
mod profile;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(profile::routes())
        .merge(admin::routes())
}
