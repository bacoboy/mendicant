terraform {
  required_version = ">= 1.15"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }
}

locals {
  app_name    = "mendicant"
  environment = "prod"
  prefix      = "${local.app_name}-${local.environment}"
  github_repo = "bacoboy/mendicant"
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

# ── GitHub OIDC ───────────────────────────────────────────────────────────────
# Allows GitHub Actions runners to assume an IAM role without static credentials.
# IAM is global — provider alias doesn't matter here.

resource "aws_iam_openid_connect_provider" "github" {
  provider = aws.us_east_2

  url            = "https://token.actions.githubusercontent.com"
  client_id_list = ["sts.amazonaws.com"]
  thumbprint_list = [
    "6938fd4d98bab03faadb97b34396831e3780aea1",
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd",
  ]
}

data "aws_iam_policy_document" "github_actions_assume" {
  provider = aws.us_east_2

  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]

    principals {
      type        = "Federated"
      identifiers = [aws_iam_openid_connect_provider.github.arn]
    }

    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }

    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${local.github_repo}:*"]
    }
  }
}

resource "aws_iam_role" "github_actions" {
  provider = aws.us_east_2

  name               = "${local.prefix}-github-actions"
  assume_role_policy = data.aws_iam_policy_document.github_actions_assume.json
}

data "aws_caller_identity" "current" {
  provider = aws.us_east_2
}

data "aws_iam_policy_document" "ecr_push" {
  provider = aws.us_east_2

  statement {
    effect    = "Allow"
    actions   = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }

  statement {
    effect = "Allow"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:InitiateLayerUpload",
      "ecr:UploadLayerPart",
      "ecr:CompleteLayerUpload",
      "ecr:PutImage",
    ]
    resources = [
      aws_ecr_repository.auth_lambda_us_east_2.arn,
      aws_ecr_repository.auth_lambda_us_west_2.arn,
      aws_ecr_repository.users_lambda_us_east_2.arn,
      aws_ecr_repository.users_lambda_us_west_2.arn,
    ]
  }
}

resource "aws_iam_role_policy" "github_actions_ecr" {
  provider = aws.us_east_2

  name   = "ecr-push"
  role   = aws_iam_role.github_actions.id
  policy = data.aws_iam_policy_document.ecr_push.json
}
