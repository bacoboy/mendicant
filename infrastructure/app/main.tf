terraform {
  required_version = ">= 1.15"

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
}

# ── Providers ─────────────────────────────────────────────────────────────────

provider "aws" {
  alias  = "us_east_2"
  region = "us-east-2"
}

provider "aws" {
  alias  = "us_west_2"
  region = "us-west-2"
}

# ── Regional deployments ───────────────────────────────────────────────────────

module "app_us_east_2" {
  source = "./modules/regional-app"

  providers = {
    aws = aws.us_east_2
  }

  app_name    = local.app_name
  environment = local.environment
  image_tag   = var.image_tag
  rp_id       = local.domain_name
  rp_origins  = "https://api.${local.domain_name},https://beta.${local.domain_name}"
  base_url    = "https://api.${local.domain_name}"
}

module "app_us_west_2" {
  source = "./modules/regional-app"

  providers = {
    aws = aws.us_west_2
  }

  app_name    = local.app_name
  environment = local.environment
  image_tag   = var.image_tag
  rp_id       = local.domain_name
  rp_origins  = "https://api.${local.domain_name},https://beta.${local.domain_name}"
  base_url    = "https://api.${local.domain_name}"
}
