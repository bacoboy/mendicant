# Mendicant

Multi-region serverless auth platform on AWS. Passkey-only authentication, role-based access control, and OAuth 2.0 Device Authorization Grant (RFC 8628) for CLI access.

## Prerequisites

- Rust toolchain (`rustup`)
- `cargo-lambda` — `cargo install cargo-lambda`
- Docker (for local DynamoDB)
- Terraform >= 1.9.0 (for deployment)

## Local Development

### 1. Start local dependencies

```bash
docker compose up -d   # DynamoDB Local on localhost:8000
```

### 2. Generate a local signing key

```bash
openssl genrsa -out dev-key.pem 2048
```

### 3. Set environment variables

```bash
export DYNAMODB_ENDPOINT_URL=http://localhost:8000
export JWT_SIGNING_KEY_PATH=$(pwd)/dev-key.pem
export RP_ID=localhost
export RP_ORIGIN=http://localhost:9000
export BASE_URL=http://localhost:9000

# Table names (must match the tables you create — see below or use Terraform)
export TABLE_USERS=users
export TABLE_CREDENTIALS=credentials
export TABLE_REFRESH_TOKENS=refresh_tokens
export TABLE_CHALLENGES=challenges
export TABLE_OAUTH_DEVICES=oauth_devices
```

### 4. Run a lambda locally

```bash
cargo lambda watch -p auth-lambda    # auth/identity lambda on http://localhost:9000
cargo lambda watch -p users-lambda  # user management lambda on http://localhost:9001
```

`cargo lambda watch` acts as a local HTTP server with hot reload.

## Building

```bash
# Check everything compiles
cargo build

# Build a specific lambda for deployment
cargo lambda build -p auth-lambda --release
cargo lambda build -p users-lambda --release
```

## Testing

### Unit tests (no dependencies required)

```bash
cargo test -p domain        # pure domain logic, no I/O
```

### Integration tests (requires DynamoDB Local)

```bash
docker compose up -d        # start DynamoDB Local first

cargo test -p db            # all repository integration tests
```

Each integration test creates its own uniquely-named tables, so tests run in parallel safely. Tables are in-memory and disappear when the container restarts.

### Run a specific test

```bash
cargo test -p domain user_id_new_is_unique
cargo test -p db put_and_get_by_id
```

### Run all tests

```bash
docker compose up -d
cargo test
```

## Workspace Layout

```
crates/
  domain/       # Core types (User, Credential, Token, DeviceGrant, Challenge).
                # No AWS or HTTP deps — fully unit-testable.
  db/           # DynamoDB repositories built on domain types.
  auth-lambda/  # WebAuthn passkeys, OAuth device flow, JWT issuance, HTML pages.
  users-lambda/ # User/account management (admin + self-service profile).

infrastructure/
  modules/
    global/     # DynamoDB Global Tables, CloudFront, Route53, KMS primary key
    regional/   # Lambda, API Gateway, S3, KMS replica — deployed per region
  environments/
    dev/
    prod/
```

## Deployment

```bash
cd infrastructure/environments/dev
terraform init
terraform plan
terraform apply
```

The `regional` module is instantiated once per region using explicit provider aliases. `us-east-2` is the designated global region.

## Environment Variables (Lambda)

| Variable | Description |
|---|---|
| `TABLE_USERS` | DynamoDB users table name |
| `TABLE_CREDENTIALS` | DynamoDB credentials table name |
| `TABLE_REFRESH_TOKENS` | DynamoDB refresh_tokens table name |
| `TABLE_CHALLENGES` | DynamoDB challenges table name (regional) |
| `TABLE_OAUTH_DEVICES` | DynamoDB oauth_devices table name (regional) |
| `KMS_SIGNING_KEY_ID` | KMS key ID/ARN for JWT signing (production) |
| `JWT_SIGNING_KEY_PATH` | Path to RSA PEM file (local dev only) |
| `RP_ID` | WebAuthn Relying Party ID (e.g. `example.com`) |
| `RP_ORIGIN` | WebAuthn origin (e.g. `https://example.com`) |
| `BASE_URL` | Public base URL for activation links |

In production, `KMS_SIGNING_KEY_ID` is used. In local dev, `JWT_SIGNING_KEY_PATH` takes precedence.

## Auth Flows

**Passkey registration/login:**
`POST /auth/register/begin` → browser `navigator.credentials.create()` → `POST /auth/register/complete` → HttpOnly JWT cookie

**OAuth device flow (CLI):**
`POST /oauth/device` → display `user_code` → poll `POST /oauth/token` → user approves at `/activate` → CLI receives access + refresh tokens

**Token verification:**
`GET /.well-known/jwks.json` — public JWK for RS256 verification. KMS Multi-Region keys mean tokens issued in any region are verifiable everywhere.
