use axum::Router;
use lambda_http::{Error, run, tracing as lambda_tracing};

mod error;
mod handlers;
mod jwt;
mod signing;
mod sse;
mod state;

use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_tracing::init_default_subscriber();

    let state = AppState::init().await?;
    let app = Router::new()
        .merge(handlers::routes())
        .with_state(state);

    run(app).await
}
