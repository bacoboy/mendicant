# Project Progress

Last updated: 2026-05-04

## Current State

Email validation flow is implemented. Registration requires:
1. User provides email + display name
2. `POST /auth/register/email` validates email uniqueness, creates 15-min token
3. User receives email link (SES integration pending; dev mode returns token in response)
4. User clicks link → `/register-confirm?token=X`
5. Passkey registration via `@passkeyRegisterWithToken(token)`
6. Login (discovery mode) unchanged
7. Profile page (`/me`) working

Ready for: SES integration and Terraform deployment work.

## Done

### Core auth-lambda
- `signing.rs` — `sign_jwt` (KMS via raw RSASSA-PKCS1-v1_5-SHA256 + local via jsonwebtoken), `public_jwk`, `verify_jwt`, `decoding_key()` pre-built at cold-start
- `state.rs` — `AppState` includes Webauthn instance (from `RP_ID`/`RP_ORIGIN` env vars) and pre-computed `DecodingKey`
- `jwt.rs` — `issue_tokens`
- `handlers/well_known.rs` — `GET /.well-known/jwks.json`
- `handlers/auth.rs` — full WebAuthn passkey registration + login (begin/complete pairs), email-first registration flow with `EmailToken`
- `handlers/oauth.rs` — RFC 8628 device flow: `device_authorize`, `device_token` (polls with pending/approved/denied), `activate_complete`
- `handlers/pages.rs` + templates — login, register, register-confirm, activate, landing page, profile page (`/me`)
- `handlers/static_files.rs` — serves `/static/passkey-plugin.js` via `include_str!`
- `static/passkey-plugin.js` — Datastar ES module plugin with `@passkeyRegister`, `@passkeyLogin`, `@registerEmail`, `@passkeyRegisterWithToken`
- Logout: `POST /logout` clears auth cookie, redirects home
- Add passkey from profile: `add_passkey_begin` / `add_passkey_complete` for authenticated users

### Email validation (Session 3)
- `domain/src/email_token.rs` — `EmailToken` type with 15-min TTL
- `db/src/email_tokens.rs` — `EmailTokenRepository` with get/put/take
- `POST /auth/register/email` handler
- `GET /register-confirm?token=X` page
- `email_tokens` table added to `setup-dynamodb-local.sh`

### Session 2 — Safari/Firefox compatibility
- Caddy reverse proxy for local HTTPS
- Extension filtering in all three `*_begin` handlers
- Duplicate email validation before WebAuthn challenge generation
- Add passkey flow from `/me`
- Admin middleware and auth cookie handling

### users-lambda
- `jwt.rs` — `build_decoding_key` + `verify`
- `middleware.rs` — `AuthUser` extractor (Bearer or cookie)
- `state.rs` — pre-computed `DecodingKey`
- `handlers/profile.rs` — `GET`/`PATCH /me`
- `handlers/admin.rs` — `GET`/`PATCH`/`DELETE /admin/users/:id`

### Admin features
- `GET /admin` — dashboard with DynamoDB table stats
- `GET /admin/tables/{slug}` — paginated table browser

## Remaining

1. **SES integration** — `POST /auth/register/email` should send email with verification link instead of returning token in response. See `docs/future-work.md`.

2. **users-lambda wiring** — wire `users-lambda` into the local proxy and Terraform so `GET/PATCH /me` is served from the right lambda.

3. **Terraform: env vars** — Lambda env vars must include `RP_ID`, `RP_ORIGIN`, `BASE_URL` in addition to table names and KMS key ID. Verify Lambda IAM role has `kms:Sign` + `kms:GetPublicKey`.

4. **Token refresh flow** — no `/oauth/refresh` endpoint yet. Refresh tokens stored in DynamoDB but no exchange endpoint.

5. **Refresh token in CLI** — device_token response returns `refresh_token` (currently the jti string, not a full JWT).

6. **Tests** — no unit or integration tests yet. `domain` and `db` crates are easily testable against DynamoDB Local.

7. **Account recovery** — no flow for locked-out users. See `docs/future-work.md`.

## Discovery Mode Implementation Notes

- `login_begin`: `start_passkey_authentication(&vec![])` with empty passkeys list signals discovery mode (empty `allowCredentials`)
- `login_complete`: extract credential ID → look up in DB → manually verify challenge nonce by parsing client data JSON and comparing to stored auth state challenge
- Cannot use webauthn-rs' `finish_passkey_authentication()` for discovery mode (requires credential in original list)
- This approach trades full signature verification for simpler discovery mode. Consider proper signature verification using the passkey's public key for production hardening.
