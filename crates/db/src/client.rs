use aws_sdk_dynamodb::Client;

/// Wraps the DynamoDB SDK client with table name configuration.
///
/// Table names are read from environment variables at construction time so
/// they can differ between dev and prod without code changes.
#[derive(Clone)]
pub struct DynamoClient {
    pub inner: Client,
    pub users_table: String,
    pub credentials_table: String,
    pub refresh_tokens_table: String,
    pub challenges_table: String,
    pub oauth_devices_table: String,
}

impl DynamoClient {
    /// Construct from an already-configured SDK client and environment variables.
    ///
    /// Expected env vars:
    ///   TABLE_USERS, TABLE_CREDENTIALS, TABLE_REFRESH_TOKENS,
    ///   TABLE_CHALLENGES, TABLE_OAUTH_DEVICES
    pub fn from_env(client: Client) -> Self {
        Self {
            inner: client,
            users_table: std::env::var("TABLE_USERS").expect("TABLE_USERS not set"),
            credentials_table: std::env::var("TABLE_CREDENTIALS")
                .expect("TABLE_CREDENTIALS not set"),
            refresh_tokens_table: std::env::var("TABLE_REFRESH_TOKENS")
                .expect("TABLE_REFRESH_TOKENS not set"),
            challenges_table: std::env::var("TABLE_CHALLENGES")
                .expect("TABLE_CHALLENGES not set"),
            oauth_devices_table: std::env::var("TABLE_OAUTH_DEVICES")
                .expect("TABLE_OAUTH_DEVICES not set"),
        }
    }
}
