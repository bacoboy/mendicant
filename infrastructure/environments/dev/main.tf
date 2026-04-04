terraform {
  required_version = ">= 1.9.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # TODO: add remote state backend (S3 + DynamoDB lock table) before first prod deploy
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

# ── Global resources (us-east-2) ─────────────────────────────────────────────

module "global" {
  source = "../../modules/global"

  providers = {
    aws = aws.us_east_2
  }

  app_name        = "mendicant"
  environment     = "dev"
  replica_regions = ["us-west-2"]
}

# ── Regional: us-east-2 ───────────────────────────────────────────────────────

module "regional_us_east_2" {
  source = "../../modules/regional"

  providers = {
    aws = aws.us_east_2
  }

  app_name    = "mendicant"
  environment = "dev"
  is_primary  = true

  kms_signing_key_arn       = module.global.kms_signing_key_arn
  users_table_name          = module.global.users_table_name
  credentials_table_name    = module.global.credentials_table_name
  refresh_tokens_table_name = module.global.refresh_tokens_table_name
}

# ── Regional: us-west-2 ───────────────────────────────────────────────────────

module "regional_us_west_2" {
  source = "../../modules/regional"

  providers = {
    aws = aws.us_west_2
  }

  app_name    = "mendicant"
  environment = "dev"
  is_primary  = false

  kms_signing_key_arn       = module.global.kms_signing_key_arn
  users_table_name          = module.global.users_table_name
  credentials_table_name    = module.global.credentials_table_name
  refresh_tokens_table_name = module.global.refresh_tokens_table_name
}
