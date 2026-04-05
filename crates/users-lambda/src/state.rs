use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;
use jsonwebtoken::DecodingKey;

use crate::jwt::build_decoding_key;

/// Shared application state, constructed once at Lambda cold-start.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    /// Pre-computed RS256 decoding key so JWT verification is I/O-free
    /// on the request path.
    pub decoding_key: DecodingKey,
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

        Ok(Self { db, decoding_key })
    }
}
