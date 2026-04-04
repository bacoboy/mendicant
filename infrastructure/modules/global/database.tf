locals {
  prefix = "${var.app_name}-${var.environment}"
}

# ── Users ────────────────────────────────────────────────────────────────────
# Global Table. PK = USER#<id>, SK = PROFILE
# GSI: email-index — looks up a user by email address.

resource "aws_dynamodb_table" "users" {
  name         = "${local.prefix}-users"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"
  range_key    = "sk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  attribute {
    name = "pk"
    type = "S"
  }

  attribute {
    name = "sk"
    type = "S"
  }

  attribute {
    name = "email"
    type = "S"
  }

  global_secondary_index {
    name            = "email-index"
    hash_key        = "email"
    projection_type = "ALL"
  }

  dynamic "replica" {
    for_each = toset(var.replica_regions)
    content {
      region_name = replica.value
    }
  }

  tags = {
    app         = var.app_name
    environment = var.environment
  }
}

# ── Credentials ──────────────────────────────────────────────────────────────
# Global Table. PK = USER#<user_id>, SK = CRED#<cred_id>
# GSI: credential-id-index — looks up the owning user from a raw credential ID
#      (needed during authentication when only the credential ID is known).

resource "aws_dynamodb_table" "credentials" {
  name         = "${local.prefix}-credentials"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"
  range_key    = "sk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  attribute {
    name = "pk"
    type = "S"
  }

  attribute {
    name = "sk"
    type = "S"
  }

  attribute {
    name = "credential_id"
    type = "S"
  }

  global_secondary_index {
    name            = "credential-id-index"
    hash_key        = "credential_id"
    projection_type = "ALL"
  }

  dynamic "replica" {
    for_each = toset(var.replica_regions)
    content {
      region_name = replica.value
    }
  }

  tags = {
    app         = var.app_name
    environment = var.environment
  }
}

# ── Refresh Tokens ───────────────────────────────────────────────────────────
# Global Table. PK = TOKEN#<jti>
# GSI: user-index — lets us revoke all tokens for a user.
# TTL attribute: expires_at (Unix timestamp).

resource "aws_dynamodb_table" "refresh_tokens" {
  name         = "${local.prefix}-refresh-tokens"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }

  attribute {
    name = "pk"
    type = "S"
  }

  attribute {
    name = "user_id"
    type = "S"
  }

  global_secondary_index {
    name            = "user-index"
    hash_key        = "user_id"
    projection_type = "ALL"
  }

  dynamic "replica" {
    for_each = toset(var.replica_regions)
    content {
      region_name = replica.value
    }
  }

  tags = {
    app         = var.app_name
    environment = var.environment
  }
}
