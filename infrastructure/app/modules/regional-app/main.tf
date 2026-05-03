terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }
}

data "aws_region" "current" {}
data "aws_caller_identity" "current" {}

locals {
  prefix = "${var.app_name}-${var.environment}"
  region = data.aws_region.current.id
}

# ── Look up foundation resources by convention ────────────────────────────────

data "aws_iam_role" "lambda_exec" {
  name = "${local.prefix}-lambda-exec-${local.region}"
}

data "aws_apigatewayv2_apis" "main" {
  name          = "${local.prefix}-api"
  protocol_type = "HTTP"
}

data "aws_ecr_repository" "auth_lambda" {
  name = "${local.prefix}-auth-lambda"
}

data "aws_ecr_repository" "users_lambda" {
  name = "${local.prefix}-users-lambda"
}

locals {
  api_gw_id     = one(data.aws_apigatewayv2_apis.main.ids)
  exec_role_arn = data.aws_iam_role.lambda_exec.arn
  ecr_auth      = data.aws_ecr_repository.auth_lambda.repository_url
  ecr_users     = data.aws_ecr_repository.users_lambda.repository_url

  lambda_env = {
    TABLE_USERS                = "${local.prefix}-users"
    TABLE_CREDENTIALS          = "${local.prefix}-credentials"
    TABLE_REFRESH_TOKENS       = "${local.prefix}-refresh-tokens"
    TABLE_CHALLENGES           = "${local.prefix}-challenges"
    TABLE_EMAIL_TOKENS         = "${local.prefix}-email-tokens"
    TABLE_OAUTH_DEVICES        = "${local.prefix}-oauth-devices"
    KMS_KEY_ARN                = "arn:aws:kms:${local.region}:${data.aws_caller_identity.current.account_id}:alias/${local.prefix}-jwt-signing"
    RP_ID                      = var.rp_id
    RP_ORIGIN                  = var.rp_origin
    BASE_URL                   = var.base_url
    ENVIRONMENT                = var.environment
    AWS_USE_DUALSTACK_ENDPOINT = "true"
  }
}
