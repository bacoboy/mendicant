# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Mendicant is a multi-region serverless platform built on AWS. This repository covers the auth/identity layer — passkey-only authentication, role-based access control, and OAuth 2.0 device flow for CLI access.

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

## Cargo Workspace

```
crates/
  domain/       # Core types and business logic. No AWS or HTTP dependencies.
  db/           # DynamoDB abstractions built on domain types.
  auth-lambda/  # WebAuthn, OAuth device flow, JWT issuance, HTML rendering.
  users-lambda/ # User and account management (admin + self-service).
```

`domain` has zero infrastructure dependencies and is fully unit-testable. `db` depends only on `domain` + `aws-sdk-dynamodb`. Lambdas depend on both.

## Local Development

```bash
docker compose up -d                    # start everything
./scripts/setup-dynamodb-local.sh       # create tables (required after every fresh start)
open https://localhost:9001             # accept cert warning once

docker compose up -d --build auth-lambda  # after code changes
docker compose logs -f auth-lambda        # view Lambda logs

cargo test                              # all tests (DynamoDB must be running)
cargo test -p domain                    # one crate
cargo test -p domain test_name          # one test
```

Two env vars control local vs AWS mode:
- `DYNAMODB_ENDPOINT_URL=http://localhost:8000` — points SDK at DynamoDB Local
- `JWT_SIGNING_KEY_PATH=/path/to/dev-key.pem` — uses local RSA key instead of KMS

See `docs/local-dev.md` for certificate renewal and port reference.

## Subdirectory Docs

Detailed context is split into subdirectory CLAUDE.md files (auto-loaded) and `docs/` (read on demand):

- `crates/CLAUDE.md` — DynamoDB schema, JWT signing, multi-region Lambda rules
- `crates/auth-lambda/CLAUDE.md` — auth flows, WebAuthn details, admin/credential endpoints
- `infrastructure/CLAUDE.md` — Terraform layout, for_each rule, CI/CD, dev-key model
- `docs/progress.md` — current implementation state and remaining work
- `docs/future-work.md` — SES integration, account recovery, upcoming features
- `docs/local-dev.md` — certificate renewal, port reference
