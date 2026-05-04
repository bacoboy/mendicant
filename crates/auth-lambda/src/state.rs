use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use db::client::DynamoClient;
use jsonwebtoken::DecodingKey;
use std::collections::HashMap;
use webauthn_rs::prelude::{Url, Webauthn, WebauthnBuilder};

use crate::mailer::Mailer;
use crate::signing::Signer;

/// Shared application state, constructed once at Lambda cold-start.
#[derive(Clone)]
pub struct AppState {
    pub db: DynamoClient,
    pub signer: Signer,
    pub mailer: Mailer,
    /// One Webauthn instance per allowed origin. WebAuthn origin validation is
    /// per-instance, so multi-origin support requires a map keyed by origin string.
    webauthn_map: HashMap<String, Webauthn>,
    /// Pre-computed RS256 decoding key so JWT verification is I/O-free
    /// on the request path.
    pub decoding_key: DecodingKey,
    /// Invite code required for new account registration. Set via INVITE_CODE env var.
    pub invite_code: String,
    /// Base URL for building links in outbound emails. Set via BASE_URL env var.
    pub base_url: String,
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
        let mailer = Mailer::from_env(&config).await?;

        let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".into());

        // RP_ORIGINS is a comma-separated list (e.g. "https://api.mendicant.io,https://beta.mendicant.io").
        // Falls back to RP_ORIGIN for local dev compatibility.
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

        let invite_code = std::env::var("INVITE_CODE")
            .unwrap_or_else(|_| "changeme".into());

        let base_url = std::env::var("BASE_URL")
            .unwrap_or_else(|_| "https://localhost:9001".into());

        Ok(Self { db, signer, mailer, webauthn_map, decoding_key, invite_code, base_url })
    }

    /// Returns the Webauthn instance for the given request origin, or None if
    /// the origin is not in the allowed list.
    pub fn webauthn_for_origin(&self, origin: &str) -> Option<&Webauthn> {
        self.webauthn_map.get(origin)
    }
}
