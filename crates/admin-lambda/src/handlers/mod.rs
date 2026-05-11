use axum::Router;
use axum::middleware::from_fn_with_state;

use crate::middleware::require_admin;
use crate::state::AppState;

#[allow(dead_code)]
pub(crate) struct NavUser {
    pub email: String,
    pub is_admin: bool,
}

mod dashboard;
mod tables;
mod users;
mod util;

/// All admin-lambda routes sit behind the `require_admin` middleware, which
/// verifies the JWT and enforces role == Administrator before any handler
/// runs. This is defense-in-depth on top of the per-handler `require_admin()`
/// calls — a future handler that forgets the check still won't be reachable
/// by non-admins.
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(dashboard::routes())
        .merge(users::routes())
        .merge(tables::routes())
        .layer(from_fn_with_state(state, require_admin))
}
