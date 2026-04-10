# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Mendicant is a multi-region serverless platform built on AWS. This repository covers the auth/identity layer first — passkey-only authentication, role-based access control, and OAuth 2.0 device flow for CLI access.

**Roles:** `Free`, `Member`, `Administrator`
**Auth method:** WebAuthn/passkeys only (no passwords)
**CLI auth:** OAuth 2.0 Device Authorization Grant (RFC 8628)

## Technology Stack

- **Backend:** Rust, AWS Lambda deployed as Docker containers
- **API:** AWS HTTP API Gateway (pay-per-request)
- **Database:** DynamoDB (pay-per-request), Global Tables for persistent data, regional tables for short-lived data
- **IaC:** Terraform `>= 1.9.0`, AWS provider `~> 6.0`
- **Frontend:** Datastar (hypermedia/SSE-driven, no custom JS), Askama for server-side HTML templates
- **JWT signing:** AWS KMS Multi-Region Keys (RS256) in production; local RSA key file in dev

## Local Development (no AWS required)

The local stack simulates the production AWS architecture exactly:
- **Caddy** (HTTPS) → **local-apigw proxy** (HTTP→Lambda event) → **Lambda container** (AWS RIE) → **DynamoDB Local**

Each HTTP request becomes one discrete Lambda invocation. Lambda REPORT lines appear in `docker compose logs auth-lambda` per request.

```bash
# 1. Start everything (builds Lambda container, starts DynamoDB, proxy, Caddy)
docker compose up -d

# 2. Access the site
open https://localhost:9001   # accept the browser cert warning (or: docker compose exec caddy caddy trust)
```

**After code changes:**
```bash
docker compose up -d --build auth-lambda   # rebuilds the Lambda image and restarts
```

**Viewing Lambda logs and REPORT output:**
```bash
docker compose logs -f auth-lambda
```

**Running tests** (DynamoDB must be running via docker compose):
```bash
cargo test                    # all tests
cargo test -p domain          # one crate
cargo test -p domain test_name  # one test
```

Two environment variables control local vs AWS mode:
- `DYNAMODB_ENDPOINT_URL=http://localhost:8000` — points SDK at DynamoDB Local
- `JWT_SIGNING_KEY_PATH=/path/to/dev-key.pem` — uses local RSA key instead of KMS

When these are absent the code uses real AWS services.

**Port reference:**
- `localhost:8000` — DynamoDB Local (also used by `cargo test`)
- `localhost:3000` — local-apigw proxy (internal, no need to access directly)
- `localhost:9000` — Caddy HTTP (for dev testing without HTTPS)
- `localhost:9001` — Caddy HTTPS (required for Safari WebAuthn)

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
| `email_tokens` | Regional | `EMAIL_TOKEN#<id>` | — | configurable |
| `oauth_devices` | Regional | `DEVICE#<code>` | — | 15 min |

Regional tables (`challenges`, `email_tokens`, `oauth_devices`) are not replicated — the auth flow always starts and completes in the same region (latency-based routing), so replication provides no benefit.

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

### Admin Dashboard

`GET /admin` — Administrator-only landing page. Calls `describe_table` for all 6 tables and shows status, approximate item count, size, and billing mode.

`GET /admin/tables/{slug}` — Paginated table browser (25 items/page, cursor-based). Slugs: `users`, `credentials`, `refresh-tokens`, `challenges`, `email-tokens`, `oauth-devices`. Each table renders domain-aware columns (e.g. AAGUIDs mapped to YubiKey product names, Unix timestamps formatted, blobs omitted). Both routes return 403 for non-Administrator sessions.

### Multi-Region Design Principles

- No hardcoded region strings in Lambda code — always read `AWS_REGION` env var at runtime
- AWS SDK clients are constructed once at Lambda cold-start, not per-request
- Sign counters use conditional writes; counter anomalies are logged rather than hard-rejected (tolerance for eventual consistency lag)
- Short-lived table entries (challenges, device codes) are regional-only — same-region routing guarantees the flow completes where it started
