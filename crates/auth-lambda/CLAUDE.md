# auth-lambda — Context for Claude Code

## Auth Flows

**Registration (email-first):**
1. `POST /auth/register/email` — validates email uniqueness, creates 15-min `EmailToken`, sends link via SES (dev: returns token in response)
2. User clicks link → `GET /register-confirm?token=X`
3. `POST /auth/register/begin` — takes `{token}`, consumes it atomically, generates WebAuthn challenge
4. Browser calls `navigator.credentials.create()`
5. `POST /auth/register/complete` — verifies assertion, stores credential, issues JWT

**Login (discovery mode):**
1. `POST /auth/login/begin` — `start_passkey_authentication(&vec![])` (empty allowCredentials = discovery mode)
2. Browser shows all passkeys for the domain
3. `POST /auth/login/complete` — extracts credential ID from response, looks up in DB, manually verifies challenge nonce, issues JWT

Discovery mode cannot use webauthn-rs' `finish_passkey_authentication()` (requires credential in original list). We manually verify the challenge nonce by parsing client data JSON and comparing to stored auth state.

**OAuth device flow (CLI):** `POST /oauth/device` → display `user_code` + activation URL → poll `POST /oauth/token` → user authenticates in browser and approves → CLI receives access + refresh tokens.

## API Design

- Web frontend endpoints: return Datastar SSE streams (`Content-Type: text/event-stream`) or full HTML
- CLI endpoints (`/oauth/*`, `/.well-known/*`): return JSON

## WebAuthn + Datastar

A small Datastar plugin (`static/passkey-plugin.js`, single `<script>` tag) adds `@passkeyRegister()`, `@passkeyLogin()`, `@registerEmail()`, and `@passkeyRegisterWithToken()` actions. All application HTML uses only `data-*` attributes — no inline or separate JS per feature.

## WebAuthn Browser Compatibility

- Requires HTTPS (Caddy handles this locally)
- `RP_ORIGIN` must match the access URL exactly
- **Do not send non-standard WebAuthn extensions** — Safari rejects them. The server filters extensions in all three `*_begin` handlers (removes `credentialProtectionPolicy`, `enforceCredentialProtectionPolicy`, `uvm`)
- Authenticator prompts may appear even for duplicate registrations; validation is server-side

**Validation order** (prevents wasted user interactions):
1. `register_begin`: check email uniqueness → return `BadRequest` immediately if taken
2. Only then generate challenge and send to browser
3. Browser shows authenticator prompt
4. `register_complete`: final server-side verification

## Admin Enrollment Flow

`GET /admin/enroll?token=<id>` — single-use enrollment for first/additional hardware key.

The `bootstrap` CLI creates an admin user and stores a single-use `AdminEnrollment` challenge:
1. `POST /admin/enroll/begin` — consumes token atomically, starts `SecurityKey` registration with `authenticatorAttachment: cross-platform`, `residentKey: preferred`, `userVerification: preferred`
2. Browser prompts for PIN (CTAP2 requirement for resident credential) then touch
3. `POST /admin/enroll/complete` — verifies, stores credential, issues JWT, redirects to `/me`

`residentKey: preferred` is essential — writes credential to key's internal storage for discovery-mode login. After enrollment, login is `userVerification: discouraged` — single touch, no PIN.

## Credential Management

- `PATCH /auth/credentials/{id}` — rename a passkey. Body: `{"nickname": "..."}`. Auth required.
- `DELETE /auth/credentials/{id}` — delete a passkey. Returns 400 if last credential (lockout prevention). Auth required.
- `/me` profile page: table of all credentials (nickname, date added, last used). Inline rename. Delete button hidden when only one credential remains.

## Admin Dashboard

- `GET /admin` — Administrator-only. Calls `describe_table` for all 6 tables, shows status/count/size/billing mode.
- `GET /admin/tables/{slug}` — Paginated table browser (25 items/page, cursor-based). Slugs: `users`, `credentials`, `refresh-tokens`, `challenges`, `email-tokens`, `oauth-devices`. Domain-aware columns (AAGUIDs → YubiKey product names, Unix timestamps formatted, blobs omitted). Returns 403 for non-Administrator.
