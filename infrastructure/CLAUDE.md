# Infrastructure — Context for Claude Code

## Terraform Layout

Two separate projects with different change cadence:

```
infrastructure/
  infra/    # Foundation — apply rarely (DNS, API GW, ECR, DynamoDB, KMS, IAM)
  app/      # Deployment — apply on every code release (Lambda + API GW routes)
  ci/       # ECR setup for CI/CD
```

**Why two projects:** Keeps `terraform plan` fast during active development. Infra changes are rare and risky; code deploys are frequent and low-risk.

App layer reads foundation resources via `data` sources by name convention (`mendicant-prod-*`) and `aws_caller_identity` — no SSM, no remote state.

- `us-east-2` is the designated global region
- No `environments/` indirection — prod only, values hardcoded in `main.tf`
- Regional module instantiated once per region using explicit provider aliases (no `for_each` across providers)

## Terraform Commands

```bash
# Foundation (apply rarely)
cd infrastructure/infra && terraform apply

# App (apply on every release)
cd infrastructure/app && terraform apply -var="image_tag=sha-<sha>"
```

## Terraform Rules

**Always use `for_each` over `count`** for all resource blocks, data sources, and module calls.

Why: Lists have ordering issues — adding/removing an element shifts indices and causes Terraform to destroy/recreate wrong resources. Sets and maps key by stable identifiers.

- Use `for_each = toset([...])` or `for_each = { key = value, ... }`
- Never use `tolist()` to index into a collection — use `one()` if exactly one is expected
- Conditionals: `for_each = var.is_primary ? toset(["enabled"]) : toset([])` not `count = var.is_primary ? 1 : 0`

**Group resources by concern** in `.tf` files, not by resource type.

## CI/CD

GitHub Actions detects which lambdas changed (via `dorny/paths-filter`) and only builds the affected ones on `ubuntu-24.04-arm` (native ARM64). Unchanged lambdas are retagged via the ECR API using `ci-<branch>` as a stable pointer — no compilation, no image transfer. Every commit gets all three `sha-<commit>` tags, so Terraform always uses a single `image_tag`.

ECR lifecycle policy: keep last 10 `sha-` tagged images, expire untagged after 1 day.

## dev-key.pem Security Model

`dev-key.pem` is a throwaway RSA key used **only** for local development:
- Mounted as a Docker volume in `docker-compose.yml` — never baked into the Lambda image
- Gitignored via `*.pem` in `.gitignore`
- Never present in production containers

Production always uses KMS (`KMS_KEY_ARN` env var). The local dev path is activated only when `JWT_SIGNING_KEY_PATH` is set — never the case in production. Never copy or embed PEM key material into Docker images or commit to git.

## Multi-Region Design

- KMS Multi-Region keys: tokens issued in any region are verifiable everywhere via `GET /.well-known/jwks.json`
- Global DynamoDB Tables: `users`, `credentials`, `refresh_tokens`
- Regional-only tables: `challenges`, `email_tokens`, `oauth_devices` (latency-based routing keeps flows in one region)
