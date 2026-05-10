# Future Work

## SES Integration

Send email verification links via AWS SES instead of returning the token in the response.

**Changes needed:**
- Update `POST /auth/register/email` to call SES `SendEmail` with link `https://{RP_ORIGIN}/register-confirm?token={token}`
- In production: response becomes `{message: "Check your email"}` (no token)
- In dev (local): continue returning token in response for testing, or log to stdout
- Add SES IAM permissions to Lambda execution role
- Terraform: SES identity + sending authorization

## Account Recovery

Users have no way to regain access if their authenticator is lost or broken. This is critical for a passwordless system.

**Proposed flow:**
1. "Trouble signing in?" link on login page
2. User enters email address
3. Recovery link sent via SES (same `EmailToken` mechanism as registration)
4. Link leads to a new-authenticator registration page (similar to `/register-confirm`)
5. Consider: time-limited lockout (retry after N minutes), IP-based hints, admin unlock fallback

Security note: email-based recovery must prevent account takeover — the recovery link should be single-use and short-lived (same as registration tokens).

## email_tokens Terraform

Add `email_tokens` table to `infrastructure/infra/modules/regional`:
- Regional table (not Global — same-region routing)
- TTL attribute on `expires_at`
- Add `TABLE_EMAIL_TOKENS` env var to Lambda configuration in `infrastructure/app`

## Admin User Management (Left-Nav Redesign)

Reorganize the admin section with left-side navigation (mirroring the profile page redesign):

- **Users section** — paginated list of all users with search/filter by email, name, role, status
- **Per-user actions** — force re-authentication (revoke all refresh tokens), suspend/unsuspend, role changes
- **Force re-auth** — calls `revoke_all_for_user` for the target user and clears their sessions; user is redirected to login on next request

Details TBD when we get to this.

## Local Dev Email Testing

Without SES configured:
- `POST /auth/register/email` returns `{token: "..."}` in response
- Navigate manually to `/register-confirm?token=...`
- Alternative: mock SES endpoint or log emails to stdout via a feature flag
