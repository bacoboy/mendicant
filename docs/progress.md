# Project Progress

Last updated: 2026-05-10 (session 8)

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

### Session 5 — Token refresh + UI polish (2026-05-10)
- `POST /auth/refresh` endpoint with refresh token rotation (revoke old JTI, issue new pair)
- Three-cookie strategy: `auth` (HttpOnly 15min), `auth_exp` (JS-readable 15min), `refresh_token` (HttpOnly 30 days)
- Silent refresh JS in `base.html` — schedules `doRefresh()` 60s before `auth_exp`, clears cookie and redirects on failure
- Logout clears all three cookies
- Removed `role` field from `RefreshToken` domain type, DB layer, and admin table UI
- Profile page widened (`page-wide`), passkey rename buttons stacked vertically
- Nav user email styled in teal to distinguish from admin/logout links
- `docker-compose.yml`: `NO_PROXY` added to all services to bypass OrbStack's HTTP proxy for inter-container calls

### Session 6 — Profile left-nav, session management, UA session labels (2026-05-10)
- `/me` redesigned with sticky left sidebar (Profile / Passkeys / Sessions sections)
- Sidebar flush-left, full viewport height; content expands freely on wide screens
- Topnav is now `position: sticky` with explicit `height: 3rem`
- Profile section: clean `dt/dd` field list; Name has inline edit (Enter/Escape supported)
- `PATCH /me` endpoint → `UserRepository::update_display_name`
- Sessions section: lists active refresh tokens only (`revoked=false AND expires_at>now`)
- Current session highlighted with teal badge; fuzzy expiry ("expires in 29 days")
- "Logout all other sessions" button → `POST /auth/sessions/revoke-others`
- `RefreshToken` gains `client_hint: Option<String>` — UA label stored once at login
- `parse_ua()` in `jwt.rs` maps User-Agent to short label ("Safari · macOS", "CLI", etc.)
- Label carried forward on token rotation; CLI device flow always stores "CLI"
- `RefreshTokenRepository::list_for_user` added (queries `user-index` GSI)
- All logout paths consolidated to `POST /auth/logout` (revokes DB token + clears cookies)

### Session 8 — Admin table browser polish, routing fix (2026-05-10)
- `UserRepository::list` fully rewritten from `Scan` to `Query` using two GSIs: `sk-email-index` (list-all + email prefix) and `role-index` (role filter ± email prefix); status is always a `FilterExpression` — no table scans
- `sk-email-index` (pk=`sk`, sk=`email`) and `role-index` (pk=`role`, sk=`email`) added to `infrastructure/infra/main.tf` and `setup-dynamodb-local.sh`
- Admin raw table browser: removed `users` and `credentials` rows (superseded by proper UI); added clickable user UUID links to `/admin/users/{id}` in remaining tables
- Dashboard table links: `users` and `credentials` now point to `/admin/users` instead of the removed raw view pages
- Table browser pagination: shows "page N · ~M total" using `describe_table` estimate; "← page 1" escape when past first page
- Logout gracefully handles missing/purged refresh tokens (swallows `NotFound` + `ConditionalCheckFailed`)
- Fixed API GW routing: `GET /admin/users` and `GET+DELETE /admin/users/{id}` were explicitly routed to `users-lambda` in `infrastructure/app`; removed those routes so they fall through to `$default` (auth-lambda)

### Session 7 — Admin user management, passkey recovery (2026-05-10)
- Admin sidebar layout added to all admin pages (`page-sidebar` + `admin-layout` / `admin-sidenav` / `admin-content`)
- `GET /admin/users` — paginated user list with search (email `contains`), role filter, status filter
- `GET /admin/users/{id}` — user detail: fields, passkey list, active session count
- `POST /admin/users/{id}/status` — activate/suspend; suspend revokes all refresh tokens
- `DELETE /admin/users/{id}` — cascade delete: revoke tokens → delete credentials → delete user
- `POST /admin/users/{id}/reset-passkey` — issues 24h `PasskeyRecovery` challenge, returns recovery URL
- `GET /recover?token=X` — recovery page (same UX as register-confirm)
- `POST /auth/recover/begin` / `POST /auth/recover/complete` — recovery flow attaches new credential to existing user
- `ChallengeType::PasskeyRecovery` added to domain + db serialization
- `CredentialRepository::delete_all_for_user` + `UserRepository::delete` added
- `UserRepository::list` extended with optional email/role/status filters (full scan when filtered)
- `@passkeyRecoverWithToken()` action added to passkey-plugin.js
- `bootstrap --reset-credentials` flag purges existing admin credentials before re-issuing enrollment URL
- Fixed pre-existing broken tests in `domain` (3-arg calls to `new_admin_enrollment`, `RefreshToken::new`)
- `askama` workspace dep updated to enable `serde-json` feature (required by `|json` filter in templates)

### Admin features
- `GET /admin` — dashboard with DynamoDB table stats
- `GET /admin/tables/{slug}` — paginated table browser

## Remaining

1. **SES integration** — `POST /auth/register/email` should send email with verification link instead of returning token in response. See `docs/future-work.md`.

2. **users-lambda wiring** — `PATCH /me` and `PATCH /admin/users/{id}` are correctly routed to `users-lambda` in API GW. `GET/DELETE /admin/users/*` were fixed (session 8) to fall through to `auth-lambda`. Local docker-compose still routes everything through auth-lambda only.

3. **Terraform: env vars** — Lambda env vars must include `RP_ID`, `RP_ORIGIN`, `BASE_URL` in addition to table names and KMS key ID. Verify Lambda IAM role has `kms:Sign` + `kms:GetPublicKey`.

4. ~~**Token refresh flow**~~ — **Done.** `POST /auth/refresh` rotates refresh token on use. Three-cookie strategy: `auth` (HttpOnly, 15min), `auth_exp` (JS-readable, 15min), `refresh_token` (HttpOnly, 30 days). Silent refresh fires 60s before expiry via `setTimeout` in `base.html`. Logout clears all three cookies.

5. **Refresh token in CLI** — device_token response returns `refresh_token` (currently the jti string, not a full JWT).

6. **Tests** — no unit or integration tests yet. `domain` and `db` crates are easily testable against DynamoDB Local.

7. **Account recovery** — no flow for locked-out users. See `docs/future-work.md`.

## Discovery Mode Implementation Notes

- `login_begin`: `start_passkey_authentication(&vec![])` with empty passkeys list signals discovery mode (empty `allowCredentials`)
- `login_complete`: extract credential ID → look up in DB → manually verify challenge nonce by parsing client data JSON and comparing to stored auth state challenge
- Cannot use webauthn-rs' `finish_passkey_authentication()` for discovery mode (requires credential in original list)
- This approach trades full signature verification for simpler discovery mode. Consider proper signature verification using the passkey's public key for production hardening.
