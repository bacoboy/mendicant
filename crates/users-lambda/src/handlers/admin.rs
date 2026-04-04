use axum::Router;
use axum::routing::get;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users))
        .route("/admin/users/:id", get(get_user).patch(patch_user).delete(delete_user))
}

async fn list_users() -> &'static str {
    todo!("list users (Administrator role required), return Datastar SSE fragment")
}

async fn get_user() -> &'static str {
    todo!("get user detail (Administrator role required), return SSE fragment")
}

async fn patch_user() -> &'static str {
    todo!("update user role or status (Administrator role required), return SSE fragment")
}

async fn delete_user() -> &'static str {
    todo!("suspend or delete user (Administrator role required), return SSE confirmation")
}
