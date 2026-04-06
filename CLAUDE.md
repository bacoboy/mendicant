# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Mendicant is a multi-region serverless platform built on AWS. This repository covers the auth/identity layer first — passkey-only authentication, role-based access control, and OAuth 2.0 device flow for CLI access.

**Roles:** `Free`, `Member`, `Administrator`
**Auth method:** WebAuthn/passkeys only (no passwords)
**CLI auth:** OAuth 2.0 Device Authorization Grant (RFC 8628)

## Technology Stack

- **Backend:** Rust, AWS Lambda via `cargo-lambda` (Docker containers later)
- **API:** AWS HTTP API Gateway (pay-per-request)
- **Database:** DynamoDB (pay-per-request), Global Tables for persistent data, regional tables for short-lived data
- **IaC:** Terraform `>= 1.9.0`, AWS provider `~> 6.0`
- **Frontend:** Datastar (hypermedia/SSE-driven, no custom JS), Askama for server-side HTML templates
- **JWT signing:** AWS KMS Multi-Region Keys (RS256) in production; local RSA key file in dev

## Local Development (no AWS required)

```bash
# Start local dependencies
docker compose up -d          # DynamoDB Local on localhost:8000

# Run a lambda with hot reload (acts as local HTTP server)
cargo lambda watch --port 8000

# Run all tests
cargo test

# Run tests for one crate
cargo test -p domain

# Run a single test
cargo test -p domain test_name

# Build a lambda (for deployment)
cargo lambda lambda build -p auth-lambda --release
```

Two environment variables control local vs AWS mode:
- `DYNAMODB_ENDPOINT_URL=http://localhost:8000` — points SDK at DynamoDB Local
- `JWT_SIGNING_KEY_PATH=/path/to/dev-key.pem` — uses local RSA key instead of KMS

When these are absent the code uses real AWS services.

### HTTPS for Local WebAuthn Testing

Safari requires HTTPS for WebAuthn (even on localhost). Use Caddy as a reverse proxy:

```bash
# Install Caddy
brew install caddy

# Create Caddyfile for HTTPS on port 9000
cat > Caddyfile << 'EOF'
localhost:9000 {
  reverse_proxy localhost:8000
}
EOF

# Run Caddy (auto-generates HTTPS certificates)
caddy run

# Set environment variables
export RP_ORIGIN=https://localhost:9000
export RP_ID=localhost
```

Then access the site at `https://localhost:9000`. Caddy auto-generates self-signed certs, so accept the browser warnings.

## Terraform

```bash
cd infrastructure/environments/dev   # or prod
terraform init
terraform plan
terraform apply
```

## Architecture

### Cargo Workspace

```
crates/
  domain/       # Core types and business logic. No AWS or HTTP dependencies.
  db/           # DynamoDB abstractions built on domain types.
  auth-lambda/  # WebAuthn, OAuth device flow, JWT issuance, HTML rendering.
  users-lambda/ # User and account management (admin + self-service).
```

`domain` has zero infrastructure dependencies and is fully unit-testable. `db` depends only on `domain` + `aws-sdk-dynamodb`. Lambdas depend on both.

JWT signing is abstracted behind a `Signer` trait with two implementations: `KmsSigner` (production) and `LocalKeySigner` (local dev). The active implementation is selected at startup from environment variables.

### DynamoDB Tables

| Table | Scope | PK | SK | TTL |
|---|---|---|---|---|
| `users` | Global Table | `USER#<id>` | `PROFILE` | — |
| `credentials` | Global Table | `USER#<id>` | `CRED#<cred_id>` | — |
| `refresh_tokens` | Global Table | `TOKEN#<jti>` | — | 30 days |
| `challenges` | Regional | `CHALLENGE#<id>` | — | 5 min |
| `oauth_devices` | Regional | `DEVICE#<code>` | — | 15 min |

`challenges` and `oauth_devices` are regional-only — the auth flow always starts and completes in the same region (latency-based routing), so replication provides no benefit.

### Terraform Layout

```
infrastructure/
  modules/
    global/     # DynamoDB global tables, CloudFront, Route53, KMS primary key
    regional/   # Lambda, HTTP API Gateway, S3, KMS replica — deployed per region
  environments/
    dev/
    prod/
```

- `us-east-2` is the designated global region
- Regional module is instantiated once per region using explicit provider aliases (no `for_each` across providers)
- Use `for_each` over maps/sets for any repeated resources within a module — never `count`
- Group resources by concern in `.tf` files, not by resource type (e.g. a Lambda + its IAM role + its API Gateway integration live in the same file)

### Auth Flows

**WebAuthn registration/login:** `begin` endpoint returns challenge via Datastar SSE (`datastar-patch-signals`), browser calls `navigator.credentials`, `complete` endpoint verifies assertion and issues JWT.

**OAuth device flow (CLI):** CLI calls `POST /oauth/device` → displays `user_code` + activation URL → polls `POST /oauth/token` → user authenticates in browser and approves → CLI receives access + refresh tokens.

**Tokens:** RS256-signed JWTs. Access tokens 15 min. Refresh tokens 30 days, stored in DynamoDB for revocation. KMS Multi-Region keys mean tokens issued in any region are verifiable everywhere via `GET /.well-known/jwks.json`.

### API Design

Endpoints serving the web frontend return Datastar SSE streams (`Content-Type: text/event-stream`) or full HTML pages. Endpoints serving the CLI (`/oauth/*`, `/.well-known/*`) return JSON.

### WebAuthn + Datastar

WebAuthn requires `navigator.credentials` browser API calls which cannot be expressed in pure HTML attributes. A small Datastar plugin (single `<script>` tag alongside the Datastar script, no app-specific code) adds `@passkeyRegister()` and `@passkeyLogin()` actions. All application HTML uses only `data-*` attributes — no inline or separate JavaScript written per-feature.

### WebAuthn Browser Compatibility

**Safari/Firefox requirements:**
- Requires HTTPS (localhost exception with proper cert setup)
- RP_ORIGIN must match the access URL exactly (e.g., `https://localhost:9000`)
- Do not send non-standard WebAuthn extensions in options (Safari rejects them) — the server filters extensions in all three `*_begin` handlers
- Browser may show authenticator prompts (Face ID, Touch ID, security key) even for duplicate registrations; validation happens server-side

**Validation order (prevent wasted user interactions):**
1. `register_begin`: Check if email already registered → return BadRequest immediately
2. Only then generate challenge and send to browser
3. Browser shows authenticator prompt
4. `register_complete`: Final server-side verification (email check redundant but safe)

### Multi-Region Design Principles

- No hardcoded region strings in Lambda code — always read `AWS_REGION` env var at runtime
- AWS SDK clients are constructed once at Lambda cold-start, not per-request
- Sign counters use conditional writes; counter anomalies are logged rather than hard-rejected (tolerance for eventual consistency lag)
- Short-lived table entries (challenges, device codes) are regional-only — same-region routing guarantees the flow completes where it started
