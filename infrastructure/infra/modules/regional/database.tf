data "aws_availability_zones" "this" {
  state = "available"
}

locals {
  prefix       = "${var.app_name}-${var.environment}"
  region       = data.aws_region.current.id
  short_region = one(toset([
    for az_id in data.aws_availability_zones.this.zone_ids :
    split("-", az_id)[0]
  ]))
}

# ── Email Tokens ──────────────────────────────────────────────────────────────
# Regional only — not replicated. Email validation tokens are short-lived and
# always consumed in the same region they were created in (latency routing).

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

# ── Challenges ────────────────────────────────────────────────────────────────
# Regional only. WebAuthn begin/complete always runs in the same region.

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
# Regional only. Device flow activation always completes in the same region.
# GSI: user-code-index — lets the browser activation page look up by user_code.

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
