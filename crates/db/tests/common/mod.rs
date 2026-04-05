use aws_sdk_dynamodb::{
    Client,
    config::{BehaviorVersion, Credentials, Region},
    types::{
        AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType,
        Projection, ProjectionType, ScalarAttributeType,
    },
};
use db::client::DynamoClient;
use uuid::Uuid;

pub struct TestEnv {
    pub db: DynamoClient,
}

// ── Builder helpers ────────────────────────────────────────────────────────────

fn attr(name: &str) -> AttributeDefinition {
    AttributeDefinition::builder()
        .attribute_name(name)
        .attribute_type(ScalarAttributeType::S)
        .build()
        .unwrap()
}

fn hash_key(name: &str) -> KeySchemaElement {
    KeySchemaElement::builder()
        .attribute_name(name)
        .key_type(KeyType::Hash)
        .build()
        .unwrap()
}

fn range_key(name: &str) -> KeySchemaElement {
    KeySchemaElement::builder()
        .attribute_name(name)
        .key_type(KeyType::Range)
        .build()
        .unwrap()
}

fn gsi(index_name: &str, hash_attr: &str) -> GlobalSecondaryIndex {
    GlobalSecondaryIndex::builder()
        .index_name(index_name)
        .key_schema(hash_key(hash_attr))
        .projection(
            Projection::builder()
                .projection_type(ProjectionType::All)
                .build(),
        )
        .build()
        .unwrap()
}

// ── TestEnv ────────────────────────────────────────────────────────────────────

impl TestEnv {
    /// Spin up a `DynamoClient` pointing at DynamoDB Local with uniquely-named
    /// tables so concurrent test binaries don't collide.
    pub async fn new() -> Self {
        let suffix = &Uuid::new_v4().simple().to_string()[..8];

        let creds = Credentials::new("test", "test", None, None, "test");
        let conf = aws_sdk_dynamodb::config::Builder::new()
            .endpoint_url("http://localhost:8000")
            .region(Region::new("us-east-1"))
            .credentials_provider(creds)
            .behavior_version(BehaviorVersion::latest())
            .build();
        let client = Client::from_conf(conf);

        let db = DynamoClient {
            inner: client.clone(),
            users_table: format!("test-users-{suffix}"),
            credentials_table: format!("test-creds-{suffix}"),
            refresh_tokens_table: format!("test-tokens-{suffix}"),
            challenges_table: format!("test-challenges-{suffix}"),
            oauth_devices_table: format!("test-devices-{suffix}"),
        };

        create_tables(&client, &db).await;
        Self { db }
    }
}

async fn create_tables(client: &Client, db: &DynamoClient) {
    // users: composite key (pk HASH, sk RANGE) + email GSI
    client
        .create_table()
        .table_name(&db.users_table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attr("pk"))
        .attribute_definitions(attr("sk"))
        .attribute_definitions(attr("email"))
        .key_schema(hash_key("pk"))
        .key_schema(range_key("sk"))
        .global_secondary_indexes(gsi("email-index", "email"))
        .send()
        .await
        .expect("create users table");

    // credentials: composite key + credential_id GSI
    client
        .create_table()
        .table_name(&db.credentials_table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attr("pk"))
        .attribute_definitions(attr("sk"))
        .attribute_definitions(attr("credential_id"))
        .key_schema(hash_key("pk"))
        .key_schema(range_key("sk"))
        .global_secondary_indexes(gsi("credential-id-index", "credential_id"))
        .send()
        .await
        .expect("create credentials table");

    // refresh_tokens: single HASH key + user_id GSI
    client
        .create_table()
        .table_name(&db.refresh_tokens_table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attr("pk"))
        .attribute_definitions(attr("user_id"))
        .key_schema(hash_key("pk"))
        .global_secondary_indexes(gsi("user-index", "user_id"))
        .send()
        .await
        .expect("create refresh_tokens table");

    // challenges: single HASH key only
    client
        .create_table()
        .table_name(&db.challenges_table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attr("pk"))
        .key_schema(hash_key("pk"))
        .send()
        .await
        .expect("create challenges table");

    // oauth_devices: single HASH key + user_code GSI
    client
        .create_table()
        .table_name(&db.oauth_devices_table)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(attr("pk"))
        .attribute_definitions(attr("user_code"))
        .key_schema(hash_key("pk"))
        .global_secondary_indexes(gsi("user-code-index", "user_code"))
        .send()
        .await
        .expect("create oauth_devices table");
}
