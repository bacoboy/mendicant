use anyhow::{Context, Result};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

use domain::token::AccessTokenClaims;

use crate::error::AppError;

/// Verify a JWT and return its claims. Uses a pre-built DecodingKey from
/// AppState so there is no I/O on the request path.
pub fn verify(token: &str, key: &DecodingKey) -> Result<AccessTokenClaims, AppError> {
    let validation = Validation::new(Algorithm::RS256);
    let data = decode::<AccessTokenClaims>(token, key, &validation)
        .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

/// Build a DecodingKey from the same env vars the signing key uses.
/// Called once at cold-start; the key is stored in AppState.
pub async fn build_decoding_key(aws_config: &aws_config::SdkConfig) -> Result<DecodingKey> {
    if let Ok(path) = std::env::var("JWT_SIGNING_KEY_PATH") {
        let pem = std::fs::read(&path)
            .with_context(|| format!("failed to read JWT_SIGNING_KEY_PATH {path}"))?;
        return decoding_key_from_private_pem(&pem);
    }

    let key_id = std::env::var("KMS_SIGNING_KEY_ID")
        .map_err(|_| anyhow::anyhow!("either JWT_SIGNING_KEY_PATH or KMS_SIGNING_KEY_ID must be set"))?;

    let client = aws_sdk_kms::Client::new(aws_config);
    let resp = client
        .get_public_key()
        .key_id(&key_id)
        .send()
        .await
        .context("KMS GetPublicKey failed")?;

    let der = resp
        .public_key
        .ok_or_else(|| anyhow::anyhow!("KMS returned no public key"))?
        .into_inner();

    decoding_key_from_der(&der)
}

fn decoding_key_from_private_pem(pem: &[u8]) -> Result<DecodingKey> {
    use rsa::pkcs8::DecodePrivateKey as _;
    use rsa::pkcs1::DecodeRsaPrivateKey as _;
    use rsa::pkcs8::EncodePublicKey as _;
    let pem_str = std::str::from_utf8(pem).context("private key PEM is not valid UTF-8")?;
    let private_key = rsa::RsaPrivateKey::from_pkcs8_pem(pem_str)
        .or_else(|_| rsa::RsaPrivateKey::from_pkcs1_pem(pem_str))
        .context("failed to parse RSA private key")?;
    let pub_key = rsa::RsaPublicKey::from(&private_key);
    let pub_pem = pub_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .context("failed to encode public key")?;
    DecodingKey::from_rsa_pem(pub_pem.as_bytes()).context("failed to build DecodingKey")
}

fn decoding_key_from_der(der: &[u8]) -> Result<DecodingKey> {
    use rsa::pkcs8::DecodePublicKey as _;
    use rsa::pkcs8::EncodePublicKey as _;
    let pub_key = rsa::RsaPublicKey::from_public_key_der(der)
        .context("failed to parse public key DER")?;
    let pem = pub_key
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .context("failed to encode public key")?;
    DecodingKey::from_rsa_pem(pem.as_bytes()).context("failed to build DecodingKey")
}
