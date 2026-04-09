locals {
  prefix = "${var.app_name}-${var.environment}"
  region = data.aws_region.current.id
}

# ── Email Tokens ─────────────────────────────────────────────────────────────
# Regional only — not replicated. Email validation tokens are short-lived and
# region-specific (registration always completes in the same region).
# TTL: 15 minutes.

resource "aws_dynamodb_table" "email_tokens" {
  name         = "${local.prefix}-email-tokens"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }

  attribute {
    name = "pk"
    type = "S"
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

# ── Challenges ───────────────────────────────────────────────────────────────
# Regional only — not replicated. The auth begin/complete flow always runs in
# the same region (latency-based routing), so cross-region access never occurs.
# TTL: 5 minutes.

resource "aws_dynamodb_table" "challenges" {
  name         = "${local.prefix}-challenges"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }

  attribute {
    name = "pk"
    type = "S"
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

# ── OAuth Device Grants ───────────────────────────────────────────────────────
# Regional only — not replicated. Device flow activation always completes in
# the same region the CLI request originated from.
# TTL: 15 minutes.
# GSI: user-code-index — browser activation page looks up grant by user_code.

resource "aws_dynamodb_table" "oauth_devices" {
  name         = "${local.prefix}-oauth-devices"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }

  attribute {
    name = "pk"
    type = "S"
  }

  attribute {
    name = "user_code"
    type = "S"
  }

  global_secondary_index {
    name            = "user-code-index"
    hash_key        = "user_code"
    projection_type = "ALL"
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}
