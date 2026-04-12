# Mendicant

Multi-region serverless auth platform on AWS. Passkey-only authentication, role-based access control, and OAuth 2.0 Device Authorization Grant (RFC 8628) for CLI access.

## Prerequisites

- Rust toolchain (`rustup`)
- Docker (for DynamoDB Local, Lambda RIE, local-apigw, Caddy)
- Terraform >= 1.9.0 (for deployment only)
- AWS CLI with local profile configured (for DynamoDB setup)

## Local Development

The local stack simulates the production AWS architecture exactly:
- **DynamoDB Local** (in-memory tables, regional-only)
- **Lambda container** (AWS Runtime Interface Emulator)
- **local-apigw proxy** (translates HTTP → API Gateway v2 events)
- **Caddy** (HTTPS reverse proxy, required for Safari WebAuthn)

Each HTTP request becomes one discrete Lambda invocation with REPORT lines in the logs.

### 1. Generate a local signing key

```bash
openssl genrsa -out dev-key.pem 2048
```

### 2. Start the full stack

```bash
docker compose up -d
```

This builds the Lambda container and starts:
- DynamoDB Local on `localhost:8000`
- Lambda RIE on `localhost:8080` (internal, via local-apigw)
- local-apigw proxy on `localhost:3000`
- Caddy on `localhost:9000` (HTTP) and `localhost:9001` (HTTPS)

### 3. Create DynamoDB tables

```bash
bash scripts/setup-dynamodb-local.sh
```

This creates the 6 tables for local development and enables TTL attributes.

### 4. Bootstrap the first administrator

The admin account is created out-of-band — not via the public registration flow. Run the bootstrap tool once after the tables are set up:

```bash
AWS_ACCESS_KEY_ID=test \
AWS_SECRET_ACCESS_KEY=test \
DYNAMODB_ENDPOINT_URL=http://localhost:8000 \
TABLE_USERS=users \
TABLE_CREDENTIALS=credentials \
TABLE_REFRESH_TOKENS=refresh_tokens \
TABLE_CHALLENGES=challenges \
TABLE_EMAIL_TOKENS=email_tokens \
TABLE_OAUTH_DEVICES=oauth_devices \
SITE_URL=https://localhost:9001 \
cargo run -p bootstrap -- admin@example.com --display-name "Admin"
```

(The dummy AWS credentials are only needed for local DynamoDB; they don't need to be real.)

The tool prints a single-use enrollment URL valid for 60 minutes (pass `--ttl-minutes N` to change). Open that URL in the browser with your hardware security key attached and click **Register Security Key**.

Enrollment requires a cross-platform authenticator (hardware key — USB/NFC). Platform authenticators (Touch ID, Face ID, Windows Hello) are rejected. The key is enrolled as a **resident/discoverable credential** (`residentKey: preferred`) so that discovery-mode login (no email required) can find it. Writing a resident credential to a PIN-protected key requires UV once — you will be prompted for your PIN during enrollment. After that, every login is a single touch with no PIN.

Re-run the tool any time to generate a fresh token (if the previous one expired or enrollment failed). The admin user record is reused — a new token is all that is issued.

**Managing passkeys** (after first enrollment): sign in and go to `/me`. The profile page shows all registered passkeys in a table. You can rename any key, delete any key (except the last one — deleting your only key locks you out), and add new passkeys.

### 5. Access the application

```bash
open https://localhost:9001
```

Accept the browser cert warning on first use, or run:
```bash
docker compose exec caddy caddy trust
```

### 6. Viewing Lambda logs

Each HTTP request produces a REPORT line showing invocation time:

```bash
docker compose logs -f auth-lambda
```

### 7. Rebuilding the Lambda after code changes

```bash
docker compose up -d --build auth-lambda
```

The `--build` flag rebuilds the Docker image before restarting. `restart` alone does not rebuild.

Build times are fast after the first build. The Dockerfile uses a two-stage stub-source pattern: manifests are copied first and all external dependencies are compiled into a cached Docker layer; real source is copied in the second stage so only workspace crates (`domain`, `db`, `auth-lambda`) are recompiled on source changes. External dependencies are never recompiled unless `Cargo.lock` changes.

## Building

```bash
# Check everything compiles
cargo build

# Rebuild and restart the Lambda container after code changes:
docker compose up -d --build auth-lambda
```

For deployment builds (see Deployment section), the Docker build process handles Lambda compilation.

## Local Stack Architecture

The Docker Compose setup locally emulates the production AWS architecture:

```
Browser (HTTPS)
    ↓
Caddy (localhost:9001)
    ↓
local-apigw proxy (localhost:3000)
    ↓
Lambda RIE (localhost:8080 — internal)
    ↓
DynamoDB Local (localhost:8000)
```

- **Caddy** provides HTTPS (required for Safari WebAuthn `navigator.credentials`)
- **local-apigw** converts HTTP requests into API Gateway v2 format, invokes Lambda, returns the response
- **Lambda RIE** runs the compiled auth-lambda binary
- **DynamoDB Local** serves in-memory tables for regional/global table testing

Each HTTP request becomes a single Lambda invocation with a REPORT line in the logs (just like AWS Lambda).

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
  bootstrap/    # CLI tool: create first admin user + emit YubiKey enrollment URL.

infrastructure/
  infra/                  # Foundation — DNS, API GW, ECR, DynamoDB, KMS, IAM
    main.tf               # Global resources inlined; calls regional module per region
    modules/regional/     # Per-region infra resources
  app/                    # Deployment — Lambda functions + API GW routes
    main.tf               # Calls regional-app module per region
    variables.tf          # image_tag variable
    modules/regional-app/ # Lambda + integrations + routes
```

## Deployment

Two separate Terraform projects with different change cadence:

```bash
# Foundation (DNS, API GW, ECR, DynamoDB, KMS, IAM) — apply rarely
cd infrastructure/infra
terraform init
terraform apply

# App (Lambda functions + routes) — apply on every release
cd infrastructure/app
terraform init
terraform apply -var="image_tag=sha-<sha>"
```

The image tag is printed by the CI build workflow after each successful push to `main`. `us-east-2` is the designated global region; `us-west-2` is a replica.

## Environment Variables (Lambda)

| Variable | Description |
|---|---|
| `TABLE_USERS` | DynamoDB users table name |
| `TABLE_CREDENTIALS` | DynamoDB credentials table name |
| `TABLE_REFRESH_TOKENS` | DynamoDB refresh_tokens table name |
| `TABLE_CHALLENGES` | DynamoDB challenges table name (regional) |
| `TABLE_EMAIL_TOKENS` | DynamoDB email_tokens table name (regional) |
| `TABLE_OAUTH_DEVICES` | DynamoDB oauth_devices table name (regional) |
| `KMS_KEY_ARN` | KMS key ARN for JWT signing (production only) |
| `JWT_SIGNING_KEY_PATH` | Path to RSA PEM file (local dev only — mounted as Docker volume, never baked into image) |
| `RP_ID` | WebAuthn Relying Party ID (e.g. `example.com`) |
| `RP_ORIGIN` | WebAuthn origin (e.g. `https://example.com`) |
| `BASE_URL` | Public base URL for activation links |
| `ENVIRONMENT` | Set to `dev` to disable HTTPS-only cookies |

In production, `KMS_SIGNING_KEY_ID` is used. In local dev, `JWT_SIGNING_KEY_PATH` takes precedence.

## Auth Flows

**Passkey registration (with email validation):**
1. `POST /auth/register/start` (email) → email token sent via SES/local
2. User confirms email via token → `/auth/register/email-confirmed`
3. `POST /auth/register/begin` (passkey challenge) → browser `navigator.credentials.create()`
4. `POST /auth/register/complete` (assertion) → HttpOnly JWT cookie

**Passkey login:**
`POST /auth/login/begin` → browser `navigator.credentials.get()` → `POST /auth/login/complete` → HttpOnly JWT cookie

**OAuth device flow (CLI):**
`POST /oauth/device` → display `user_code` → poll `POST /oauth/token` → user approves at `/activate` → CLI receives access + refresh tokens

**Admin hardware key enrollment (first-run only):**
Run `cargo run -p bootstrap -- <email>` → copy the printed URL → open it in a browser with hardware security key attached → enter PIN (once, to write the resident credential) → touch the key. The bootstrap tool writes the admin user directly to DynamoDB. Hardware enforcement is via `authenticatorAttachment: cross-platform` (rejects platform authenticators). After enrollment, login is a single touch — no PIN.

**Token verification:**
`GET /.well-known/jwks.json` — public JWK for RS256 verification. KMS Multi-Region keys mean tokens issued in any region are verifiable everywhere.

**Admin dashboard:**
`GET /admin` — table overview (item counts, size, billing mode). `GET /admin/tables/{slug}` — paginated contents with domain-aware columns. Both require `Administrator` role; non-admin requests return 403.
