use anyhow::{Context, Result};
use base64::Engine as _;
use serde::Serialize;

use aws_sdk_kms::primitives::Blob;
use aws_sdk_kms::types::{MessageType, SigningAlgorithmSpec};
use jsonwebtoken::{Algorithm, EncodingKey, Header, Validation, DecodingKey, decode, encode};

use domain::token::AccessTokenClaims;

/// JWT signing abstraction — switches between KMS (production) and a local
/// RSA key file (local dev) based on environment variables.
///
/// Set JWT_SIGNING_KEY_PATH to a PEM file path to use local signing.
/// Otherwise, set KMS_SIGNING_KEY_ID to a KMS key ARN/alias.
#[derive(Clone)]
pub enum Signer {
    Kms(KmsSigner),
    Local(LocalSigner),
}

#[derive(Clone)]
pub struct KmsSigner {
    pub client: aws_sdk_kms::Client,
    pub key_id: String,
}

#[derive(Clone)]
pub struct LocalSigner {
    /// PEM-encoded RSA private key bytes.
    pub private_key_pem: Vec<u8>,
}

impl Signer {
    pub async fn from_env(aws_config: &aws_config::SdkConfig) -> Result<Self> {
        if let Ok(path) = std::env::var("JWT_SIGNING_KEY_PATH") {
            let pem = std::fs::read(&path)
                .map_err(|e| anyhow::anyhow!("failed to read JWT_SIGNING_KEY_PATH {path}: {e}"))?;
            return Ok(Self::Local(LocalSigner { private_key_pem: pem }));
        }

        let key_id = std::env::var("KMS_SIGNING_KEY_ID")
            .map_err(|_| anyhow::anyhow!("either JWT_SIGNING_KEY_PATH or KMS_SIGNING_KEY_ID must be set"))?;

        let client = aws_sdk_kms::Client::new(aws_config);
        Ok(Self::Kms(KmsSigner { client, key_id }))
    }

    /// Sign a JWT payload and return the compact serialized token.
    pub async fn sign_jwt<S: Serialize + Send>(&self, claims: &S) -> Result<String> {
        match self {
            Self::Kms(kms) => kms.sign_jwt(claims).await,
            Self::Local(local) => local.sign_jwt(claims),
        }
    }

    /// Return the public key in JWK format for the JWKS endpoint.
    pub async fn public_jwk(&self) -> Result<serde_json::Value> {
        match self {
            Self::Kms(kms) => kms.public_jwk().await,
            Self::Local(local) => local.public_jwk(),
        }
    }

    /// Pre-compute and return a DecodingKey for JWT verification.
    /// Called once at cold-start; the result is stored in AppState.
    pub async fn decoding_key(&self) -> Result<DecodingKey> {
        match self {
            Self::Kms(kms) => {
                let resp = kms.client
                    .get_public_key()
                    .key_id(&kms.key_id)
                    .send()
                    .await
                    .context("KMS GetPublicKey failed")?;
                let der = resp
                    .public_key
                    .ok_or_else(|| anyhow::anyhow!("KMS returned no public key"))?
                    .into_inner();
                decoding_key_from_der(&der)
            }
            Self::Local(local) => {
                decoding_key_from_private_pem(&local.private_key_pem)
            }
        }
    }
}

// ── KMS ───────────────────────────────────────────────────────────────────────

impl KmsSigner {
    async fn sign_jwt<S: Serialize + Send>(&self, claims: &S) -> Result<String> {
        let kid = short_kid(&self.key_id);
        let header_val = serde_json::json!({"alg": "RS256", "typ": "JWT", "kid": kid});
        let header = b64url_json(&header_val)?;
        let payload = b64url_json(&serde_json::to_value(claims)?)?;
        let signing_input = format!("{header}.{payload}");

        let resp = self.client
            .sign()
            .key_id(&self.key_id)
            .message(Blob::new(signing_input.as_bytes().to_vec()))
            .message_type(MessageType::Raw)
            .signing_algorithm(SigningAlgorithmSpec::RsassaPkcs1V15Sha256)
            .send()
            .await
            .context("KMS Sign call failed")?;

        let sig_bytes = resp
            .signature
            .ok_or_else(|| anyhow::anyhow!("KMS returned no signature"))?
            .into_inner();

        let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&sig_bytes);
        Ok(format!("{signing_input}.{sig}"))
    }

    async fn public_jwk(&self) -> Result<serde_json::Value> {
        use rsa::pkcs8::DecodePublicKey as _;

        let resp = self.client
            .get_public_key()
            .key_id(&self.key_id)
            .send()
            .await
            .context("KMS GetPublicKey failed")?;

        let der = resp
            .public_key
            .ok_or_else(|| anyhow::anyhow!("KMS returned no public key"))?
            .into_inner();

        let pub_key = rsa::RsaPublicKey::from_public_key_der(&der)
            .context("failed to parse KMS public key DER")?;

        Ok(rsa_to_jwk(&pub_key, &short_kid(&self.key_id)))
    }
}

// ── Local ─────────────────────────────────────────────────────────────────────

impl LocalSigner {
    fn sign_jwt<S: Serialize + Send>(&self, claims: &S) -> Result<String> {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("dev".into());
        let key = EncodingKey::from_rsa_pem(&self.private_key_pem)
            .context("invalid RSA private key PEM")?;
        Ok(encode(&header, claims, &key)?)
    }

    fn public_jwk(&self) -> Result<serde_json::Value> {
        let pub_key = extract_public_key(&self.private_key_pem)?;
        Ok(rsa_to_jwk(&pub_key, "dev"))
    }
}

// ── JWT verification ──────────────────────────────────────────────────────────

/// Verify a JWT and return the decoded claims. Uses a pre-built DecodingKey
/// from AppState so there is no I/O on the hot path.
pub fn verify_jwt(token: &str, key: &DecodingKey) -> Result<AccessTokenClaims> {
    let validation = Validation::new(Algorithm::RS256);
    let data = decode::<AccessTokenClaims>(token, key, &validation)
        .context("JWT verification failed")?;
    Ok(data.claims)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn b64url_json(value: &serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes))
}

fn rsa_to_jwk(key: &rsa::RsaPublicKey, kid: &str) -> serde_json::Value {
    use rsa::traits::PublicKeyParts as _;
    let n = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.n().to_bytes_be());
    let e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key.e().to_bytes_be());
    serde_json::json!({
        "kty": "RSA",
        "alg": "RS256",
        "use": "sig",
        "kid": kid,
        "n": n,
        "e": e,
    })
}

/// Derive a short, stable kid from a KMS key ARN or alias.
fn short_kid(key_id: &str) -> String {
    key_id.split('/').last().unwrap_or(key_id).to_string()
}

fn extract_public_key(private_key_pem: &[u8]) -> Result<rsa::RsaPublicKey> {
    use rsa::pkcs8::DecodePrivateKey as _;
    use rsa::pkcs1::DecodeRsaPrivateKey as _;
    let pem = std::str::from_utf8(private_key_pem)
        .context("private key PEM is not valid UTF-8")?;
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| rsa::RsaPrivateKey::from_pkcs1_pem(pem))
        .context("failed to parse RSA private key (tried PKCS#8 and PKCS#1)")?;
    Ok(rsa::RsaPublicKey::from(&private_key))
}

fn decoding_key_from_private_pem(pem: &[u8]) -> Result<DecodingKey> {
    use rsa::pkcs8::EncodePublicKey as _;
    let pub_key = extract_public_key(pem)?;
    let pub_pem = pub_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .context("failed to encode public key as PEM")?;
    DecodingKey::from_rsa_pem(pub_pem.as_bytes()).context("failed to build DecodingKey from PEM")
}

fn decoding_key_from_der(der: &[u8]) -> Result<DecodingKey> {
    use rsa::pkcs8::DecodePublicKey as _;
    use rsa::pkcs8::EncodePublicKey as _;
    let pub_key = rsa::RsaPublicKey::from_public_key_der(der)
        .context("failed to parse public key DER")?;
    let pem = pub_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .context("failed to encode public key as PEM")?;
    DecodingKey::from_rsa_pem(pem.as_bytes()).context("failed to build DecodingKey from DER")
}
