# Crates — Context for Claude Code

## DynamoDB Tables

| Table | Scope | PK | SK | TTL |
|---|---|---|---|---|
| `users` | Global Table | `USER#<id>` | `PROFILE` | — |
| `credentials` | Global Table | `USER#<id>` | `CRED#<cred_id>` | — |
| `refresh_tokens` | Global Table | `TOKEN#<jti>` | — | 30 days |
| `challenges` | Regional | `CHALLENGE#<id>` | — | 5 min |
| `email_tokens` | Regional | `EMAIL_TOKEN#<id>` | — | 15 min |
| `oauth_devices` | Regional | `DEVICE#<code>` | — | 15 min |

Regional tables (`challenges`, `email_tokens`, `oauth_devices`) are not replicated — same-region routing ensures each flow starts and completes in the same region.

## JWT Signing

JWT signing is abstracted behind a `Signer` trait:
- `KmsSigner` — production, uses `KMS_KEY_ARN` env var
- `LocalKeySigner` — local dev, uses `JWT_SIGNING_KEY_PATH` env var

`DecodingKey` is pre-computed at Lambda cold-start (KMS `GetPublicKey` → DER → PEM → `DecodingKey`) to keep the hot path I/O-free. RS256, access tokens 15 min, refresh tokens 30 days.

## Multi-Region Lambda Rules

- Never hardcode region strings — always read `AWS_REGION` env var at runtime
- Construct AWS SDK clients once at cold-start, not per-request
- Sign counters use conditional writes; counter anomalies are logged, not hard-rejected (tolerance for eventual consistency lag)
- Short-lived table entries (challenges, device codes) are regional-only — same-region routing handles this

## Key Design Notes

- `Credential.public_key` stores the full serialized webauthn-rs `Passkey` JSON (not raw CBOR public key bytes). AAGUID stored as `Uuid::nil()` since webauthn-rs 0.5 doesn't expose it cleanly.
- KMS signing sends raw `header.payload` bytes with `MessageType::Raw` and `SigningAlgorithmSpec::RsassaPkcs1V15Sha256` (KMS does the SHA-256 hash).
- Registration challenge bundles `{email, display_name, state: PasskeyRegistration}` as JSON in `Challenge.state_json` to prevent identity swapping between begin/complete.
