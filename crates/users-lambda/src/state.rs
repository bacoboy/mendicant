use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;

#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
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

        Ok(Self {
            db: DynamoClient::from_env(ddb_client),
        })
    }
}
