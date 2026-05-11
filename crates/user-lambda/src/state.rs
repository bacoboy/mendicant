use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;
use jsonwebtoken::DecodingKey;
use std::collections::HashMap;
use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

use crate::jwt::build_decoding_key;

/// Shared application state, constructed once at Lambda cold-start.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    /// One Webauthn instance per allowed origin (for the passkey-add flow).
    webauthn_map: HashMap<String, Webauthn>,
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

        let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".into());
        let origins_raw = std::env::var("RP_ORIGINS")
            .or_else(|_| std::env::var("RP_ORIGIN"))
            .unwrap_or_else(|_| "http://localhost:9000".into());

        let mut webauthn_map = HashMap::new();
        for origin in origins_raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let url = Url::parse(origin)
                .with_context(|| format!("invalid origin in RP_ORIGINS: {origin}"))?;
            let wa = WebauthnBuilder::new(&rp_id, &url)
                .context("failed to build Webauthn instance")?
                .rp_name("Mendicant")
                .build()
                .context("failed to finalise Webauthn instance")?;
            webauthn_map.insert(origin.to_string(), wa);
        }

        Ok(Self { db, webauthn_map, decoding_key })
    }

    /// Returns the Webauthn instance for the given request origin, or None if
    /// the origin is not in the allowed list.
    pub fn webauthn_for_origin(&self, origin: &str) -> Option<&Webauthn> {
        self.webauthn_map.get(origin)
    }
}
