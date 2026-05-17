# Future Work

## SES Production Access

SES is currently in sandbox mode — can only send to verified addresses. Request production access before opening registration to real users.

**How to request:**
AWS Console → SES → Account dashboard → Request production access.

**Form answers:**
- Mail type: Transactional
- Website URL: `mendicant.io`
- Use case: *We send transactional email only (account verification links). We use the AWS SES account-level suppression list to automatically suppress bounced and complained-about addresses. Our invite-only model strictly limits sending volume and recipient pool.*
- Expected daily volume: (whatever's realistic — err low)

**After approval:**
- Remove all entries from `local.ses_verified_emails` in `ses.tf` and delete the `aws_sesv2_email_identity.verified_emails` resource block — individual recipient verification is only needed in sandbox mode. Adding/removing addresses from the list is how sandbox recipients are managed in the interim.

## Account Recovery

Users have no way to regain access if their authenticator is lost or broken. This is critical for a passwordless system.

**Proposed flow:**
1. "Trouble signing in?" link on login page
2. User enters email address
3. Recovery link sent via SES (same `EmailToken` mechanism as registration)
4. Link leads to a new-authenticator registration page (similar to `/register-confirm`)
5. Consider: time-limited lockout (retry after N minutes), IP-based hints, admin unlock fallback

Security note: email-based recovery must prevent account takeover — the recovery link should be single-use and short-lived (same as registration tokens).

## Local Dev Email Testing

Without SES configured (`SES_FROM_ADDRESS` unset), verification links are logged via `tracing::info!`. To test locally:
- Check Lambda logs: `docker compose logs -f auth-lambda`
- Copy the URL from the log line and navigate to it manually
