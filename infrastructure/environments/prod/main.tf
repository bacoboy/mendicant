terraform {
  required_version = ">= 1.9.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # TODO: S3 backend for remote state
}

# ── Providers ────────────────────────────────────────────────────────────────

provider "aws" {
  alias  = "us_east_2"
  region = "us-east-2"
}

provider "aws" {
  alias  = "us_west_2"
  region = "us-west-2"
}

# TODO: add more regional providers as needed (eu-west-1, ap-southeast-1, etc.)

# ── Global resources ─────────────────────────────────────────────────────────

module "global" {
  source = "../../modules/global"

  providers = {
    aws = aws.us_east_2
  }

  app_name        = "mendicant"
  environment     = "prod"
  replica_regions = ["us-west-2"]
  domain_name     = var.domain_name
}

# ── Regional deployments ──────────────────────────────────────────────────────

module "regional_us_east_2" {
  source = "../../modules/regional"

  providers = {
    aws = aws.us_east_2
  }

  app_name    = "mendicant"
  environment = "prod"
  is_primary  = true

  kms_signing_key_arn       = module.global.kms_signing_key_arn
  users_table_name          = module.global.users_table_name
  credentials_table_name    = module.global.credentials_table_name
  refresh_tokens_table_name = module.global.refresh_tokens_table_name
}

module "regional_us_west_2" {
  source = "../../modules/regional"

  providers = {
    aws = aws.us_west_2
  }

  app_name    = "mendicant"
  environment = "prod"
  is_primary  = false

  kms_signing_key_arn       = module.global.kms_signing_key_arn
  users_table_name          = module.global.users_table_name
  credentials_table_name    = module.global.credentials_table_name
  refresh_tokens_table_name = module.global.refresh_tokens_table_name
}
