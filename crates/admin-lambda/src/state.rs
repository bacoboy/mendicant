use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;
use jsonwebtoken::DecodingKey;

use crate::jwt::build_decoding_key;

/// Shared application state, constructed once at Lambda cold-start.
///
/// admin-lambda does not sign JWTs (it never issues tokens) and does not run
/// WebAuthn ceremonies, so its state is small: just the DDB client + the
/// pre-computed RS256 decoding key for verifying incoming bearer tokens.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    pub decoding_key: DecodingKey,
    pub base_url: String,
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;

        let ddb_client = if let Ok(endpoint) = std::env::var("DYNAMODB_ENDPOINT_URL") {
            let ddb_config = aws_sdk_dynamodb::config::Builder::from(&config)
                .endpoint_url(endpoint)
                .build();
            Client::from_conf(ddb_config)
        } else {
            Client::new(&config)
        };

        let db = DynamoClient::from_env(ddb_client);
        let decoding_key = build_decoding_key(&config).await?;

        let base_url = std::env::var("BASE_URL")
            .unwrap_or_else(|_| "https://localhost:9001".into());

        Ok(Self { db, decoding_key, base_url })
    }
}
