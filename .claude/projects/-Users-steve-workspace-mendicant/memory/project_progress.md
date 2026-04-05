---
name: auth-lambda implementation progress
description: Where we left off implementing the auth-lambda handlers in step 6
type: project
---

In the middle of implementing step 6: auth-lambda handlers.

**Why:** Building the WebAuthn registration/login flows and OAuth device flow for the auth Lambda.

**How to apply:** Pick up from here in next conversation — implement the remaining files listed below before moving to users-lambda.

## Completed so far (this session)

- `CLAUDE.md` ✓
- Cargo workspace + all 4 crate skeletons ✓
- `domain` crate — all types (User, Role, Credential, Challenge, RefreshToken, DeviceGrant) ✓
- `db` crate — all 5 repositories fully implemented ✓
- Terraform global + regional modules + dev/prod environments ✓
- `docker-compose.yml` for DynamoDB Local ✓
- `crates/auth-lambda/src/sse.rs` — Datastar SSE response builder ✓
- `crates/auth-lambda/src/jwt.rs` — token issuance skeleton ✓

## In progress (was mid-implementation when session ended)

Just started writing `auth-lambda` handlers. The following files still need to be written:

1. `crates/auth-lambda/src/signing.rs` — implement `LocalSigner::sign_jwt` and `LocalSigner::public_jwk` using `jsonwebtoken` + `rsa` crates. KMS impl stays as `todo!()`.

2. `crates/auth-lambda/src/state.rs` — add `webauthn: Arc<Webauthn>`, all repository fields (`users`, `credentials`, `challenges`, `refresh_tokens`, `oauth_devices`), init from env vars `RP_ID`, `RP_ORIGIN`, `RP_NAME`.

3. `crates/auth-lambda/src/main.rs` — add `mod sse; mod jwt;`

4. `crates/auth-lambda/src/handlers/auth.rs` — implement all 4 handlers:
   - `register_begin`: POST body `{email, display_name}` → SSE with WebAuthn creation options + challenge_id signal
   - `register_complete`: POST body `{challenge_id, credential: RegisterPublicKeyCredential}` → verify, store User+Credential, issue JWT, set cookie, SSE redirect
   - `login_begin`: POST body `{email}` → SSE with WebAuthn request options + challenge_id signal
   - `login_complete`: POST body `{challenge_id, credential: PublicKeyCredential}` → verify, update sign_count, issue JWT, set cookie, SSE redirect

5. `crates/auth-lambda/src/handlers/oauth.rs` — implement OAuth 2.0 Device Authorization Grant (RFC 8628):
   - `device_authorize`: POST → JSON `{device_code, user_code, verification_uri, expires_in, interval}`
   - `device_token`: POST body `{grant_type, device_code}` → JSON (pending/approved/denied)
   - `activate_complete`: POST body `{user_code}` (authenticated) → approve grant, SSE confirmation

6. `crates/auth-lambda/src/handlers/well_known.rs` — JWKS endpoint using `Signer::public_jwk()`

7. `crates/auth-lambda/src/handlers/pages.rs` — Askama HTML templates for login, register, activate pages

8. `crates/auth-lambda/templates/login.html` — Datastar-based login page
9. `crates/auth-lambda/templates/register.html` — registration page
10. `crates/auth-lambda/templates/activate.html` — OAuth device activation page

## Key design decisions to remember

- webauthn-rs `Passkey` struct (serialized JSON) is stored as `Credential.public_key: Vec<u8>`
- `CredentialId(String)` = base64url encoding of webauthn-rs `CredentialID` (`Vec<u8>`)
- `Passkey::cred_id()` returns `&CredentialID` — use `base64url::encode` for our string ID
- AAGUID: use `Uuid::nil()` since webauthn-rs 0.5 doesn't expose it publicly — mark as TODO
- Datastar SSE begin response patches signals: `{challengeId: "...", webauthnOptions: {...}}`
- Auth complete: set `Set-Cookie` header on SSE response + `datastar-execute-script` redirect
- JWT cookie: `auth=<token>; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=900`
- Local signing: use `jsonwebtoken` crate with `EncodingKey::from_rsa_pem`
- Env vars for WebAuthn: `RP_ID`, `RP_ORIGIN`, `RP_NAME`
- `rsa` crate added to workspace for JWK generation from PEM

## Also pending (interrupted mid-session)

- `.gitignore` — user asked for this, needs: `target/`, `.terraform/`, `*.tfstate`, `*.tfstate.backup`, `.terraform.lock.hcl`, `*.pem`, `.env`

## Next steps after auth-lambda

- Step 7: users-lambda handlers (profile + admin)
- Step 8: Datastar passkey plugin (JavaScript, ~30 lines)
- Step 9: local dev setup docs + test with DynamoDB Local
