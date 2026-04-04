use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;

use crate::signing::Signer;

/// Shared application state, constructed once at Lambda cold-start.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    pub signer: Signer,
}

impl AppState {
    pub async fn init() -> Result<Self> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;

        // Allow DYNAMODB_ENDPOINT_URL to override for local dev.
        let ddb_client = if let Ok(endpoint) = std::env::var("DYNAMODB_ENDPOINT_URL") {
            let ddb_config = aws_sdk_dynamodb::config::Builder::from(&config)
                .endpoint_url(endpoint)
                .build();
            Client::from_conf(ddb_config)
        } else {
            Client::new(&config)
        };

        let db = DynamoClient::from_env(ddb_client);
        let signer = Signer::from_env(&config).await?;

        Ok(Self { db, signer })
    }
}
