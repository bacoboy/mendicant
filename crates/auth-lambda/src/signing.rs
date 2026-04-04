use anyhow::Result;

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
    pub async fn sign_jwt(&self, _claims: &serde_json::Value) -> Result<String> {
        match self {
            Self::Kms(_kms) => {
                todo!("sign JWT via KMS RS256")
            }
            Self::Local(_local) => {
                todo!("sign JWT with local RSA key")
            }
        }
    }

    /// Return the public key in JWK format for the JWKS endpoint.
    pub async fn public_jwk(&self) -> Result<serde_json::Value> {
        match self {
            Self::Kms(_kms) => {
                todo!("fetch public key from KMS and format as JWK")
            }
            Self::Local(_local) => {
                todo!("extract public key from local PEM and format as JWK")
            }
        }
    }
}
