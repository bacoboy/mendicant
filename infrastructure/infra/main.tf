terraform {
  required_version = ">= 1.9.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # TODO: S3 backend + DynamoDB lock table before team use
}

locals {
  app_name    = "mendicant"
  environment = "prod"
  domain_name = "mendicant.io"
  prefix      = "${local.app_name}-${local.environment}"
}

# ── Providers ─────────────────────────────────────────────────────────────────

provider "aws" {
  alias  = "us_east_2"
  region = "us-east-2"

  assume_role {
    role_arn = "arn:aws:iam::054297229654:role/Admin"
  }
}

provider "aws" {
  alias  = "us_west_2"
  region = "us-west-2"

  assume_role {
    role_arn = "arn:aws:iam::054297229654:role/Admin"
  }
}

# ── Route53 ───────────────────────────────────────────────────────────────────
# Hosted zone must exist (registered separately via Route53 or transferred).

data "aws_route53_zone" "main" {
  provider = aws.us_east_2
  name     = local.domain_name
}

# ── KMS — JWT signing key (primary, us-east-2) ────────────────────────────────
# RS4096 multi-region key. The primary lives in us-east-2.
# Each regional module creates a local replica so Lambdas sign without
# cross-region KMS calls. All regions share the same public key.

resource "aws_kms_key" "jwt_signing" {
  provider = aws.us_east_2

  description              = "${local.prefix} JWT signing key (RS256)"
  key_usage                = "SIGN_VERIFY"
  customer_master_key_spec = "RSA_4096"
  multi_region             = true
  enable_key_rotation      = false # not supported for asymmetric keys

  tags = {
    app         = local.app_name
    environment = local.environment
  }
}

resource "aws_kms_alias" "jwt_signing" {
  provider = aws.us_east_2

  name          = "alias/${local.prefix}-jwt-signing"
  target_key_id = aws_kms_key.jwt_signing.key_id
}

# ── DynamoDB Global Tables ─────────────────────────────────────────────────────
# Long-lived data replicated to all regions. Primary in us-east-2.

resource "aws_dynamodb_table" "users" {
  provider = aws.us_east_2

  name         = "${local.prefix}-users"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"
  range_key    = "sk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  attribute { name = "pk"; type = "S" }
  attribute { name = "sk"; type = "S" }
  attribute { name = "email"; type = "S" }

  global_secondary_index {
    name            = "email-index"
    hash_key        = "email"
    projection_type = "ALL"
  }

  replica { region_name = "us-west-2" }

  tags = {
    app         = local.app_name
    environment = local.environment
  }
}

resource "aws_dynamodb_table" "credentials" {
  provider = aws.us_east_2

  name         = "${local.prefix}-credentials"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"
  range_key    = "sk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  attribute { name = "pk"; type = "S" }
  attribute { name = "sk"; type = "S" }
  attribute { name = "credential_id"; type = "S" }

  global_secondary_index {
    name            = "credential-id-index"
    hash_key        = "credential_id"
    projection_type = "ALL"
  }

  replica { region_name = "us-west-2" }

  tags = {
    app         = local.app_name
    environment = local.environment
  }
}

resource "aws_dynamodb_table" "refresh_tokens" {
  provider = aws.us_east_2

  name         = "${local.prefix}-refresh-tokens"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "pk"

  stream_enabled   = true
  stream_view_type = "NEW_AND_OLD_IMAGES"

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }

  attribute { name = "pk"; type = "S" }
  attribute { name = "user_id"; type = "S" }

  global_secondary_index {
    name            = "user-index"
    hash_key        = "user_id"
    projection_type = "ALL"
  }

  replica { region_name = "us-west-2" }

  tags = {
    app         = local.app_name
    environment = local.environment
  }
}

# ── Regional resources ────────────────────────────────────────────────────────
# Each call creates: KMS replica, regional DynamoDB tables, API Gateway,
# ECR repos, IAM execution role, and SSM parameters for the app layer.

module "regional_us_east_2" {
  source = "./modules/regional"

  providers = {
    aws = aws.us_east_2
  }

  app_name    = local.app_name
  environment = local.environment
  is_primary  = true

  kms_signing_key_arn       = aws_kms_key.jwt_signing.arn
  users_table_name          = aws_dynamodb_table.users.name
  credentials_table_name    = aws_dynamodb_table.credentials.name
  refresh_tokens_table_name = aws_dynamodb_table.refresh_tokens.name
  domain_name               = local.domain_name
  route53_zone_id           = data.aws_route53_zone.main.zone_id
}

module "regional_us_west_2" {
  source = "./modules/regional"

  providers = {
    aws = aws.us_west_2
  }

  app_name    = local.app_name
  environment = local.environment
  is_primary  = false

  kms_signing_key_arn       = aws_kms_key.jwt_signing.arn
  users_table_name          = aws_dynamodb_table.users.name
  credentials_table_name    = aws_dynamodb_table.credentials.name
  refresh_tokens_table_name = aws_dynamodb_table.refresh_tokens.name
  domain_name               = local.domain_name
  route53_zone_id           = data.aws_route53_zone.main.zone_id
}
