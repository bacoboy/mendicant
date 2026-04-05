use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;
use jsonwebtoken::DecodingKey;
use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

use crate::signing::Signer;

/// Shared application state, constructed once at Lambda cold-start.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    pub signer: Signer,
    pub webauthn: Webauthn,
    /// Pre-computed RS256 decoding key so JWT verification is I/O-free
    /// on the request path.
    pub decoding_key: DecodingKey,
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
        let decoding_key = signer.decoding_key().await?;

        let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".into());
        let rp_origin = std::env::var("RP_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:9000".into());
        let rp_origin_url = Url::parse(&rp_origin)
            .with_context(|| format!("invalid RP_ORIGIN: {rp_origin}"))?;
        let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin_url)
            .context("failed to build Webauthn instance")?
            .rp_name("Mendicant")
            .build()
            .context("failed to finalise Webauthn instance")?;

        Ok(Self { db, signer, webauthn, decoding_key })
    }
}
