use axum::Router;

use crate::state::AppState;

mod auth;
mod oauth;
mod pages;
mod static_files;
mod well_known;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(pages::routes())
        .merge(auth::routes())
        .merge(oauth::routes())
        .merge(well_known::routes())
        .merge(static_files::routes())
}
