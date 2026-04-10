# HTTP API Gateway + Lambda functions + IAM execution roles for this region.
# All three are tightly coupled: the API routes to the Lambda, and the Lambda
# needs an IAM role to access DynamoDB and KMS.

locals {
  # region local is defined in database.tf
  table_names = {
    users          = var.users_table_name
    credentials    = var.credentials_table_name
    refresh_tokens = var.refresh_tokens_table_name
    email_tokens   = aws_dynamodb_table.email_tokens.name
    challenges     = aws_dynamodb_table.challenges.name
    oauth_devices  = aws_dynamodb_table.oauth_devices.name
  }

  # Common environment variables shared by both Lambda functions.
  # Reference as: environment { variables = local.lambda_env }
  lambda_env = {
    TABLE_USERS                = local.table_names.users
    TABLE_CREDENTIALS          = local.table_names.credentials
    TABLE_REFRESH_TOKENS       = local.table_names.refresh_tokens
    TABLE_CHALLENGES           = local.table_names.challenges
    TABLE_EMAIL_TOKENS         = local.table_names.email_tokens
    TABLE_OAUTH_DEVICES        = local.table_names.oauth_devices
    AWS_USE_DUALSTACK_ENDPOINT = "true"
  }
}

# ── IAM execution role (shared by both Lambdas) ───────────────────────────────

data "aws_iam_policy_document" "lambda_assume_role" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "lambda_exec" {
  name               = "${local.prefix}-lambda-exec-${local.short_region}"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "lambda_basic" {
  role       = aws_iam_role.lambda_exec.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

data "aws_iam_policy_document" "lambda_dynamodb" {
  statement {
    effect = "Allow"
    actions = [
      "dynamodb:GetItem",
      "dynamodb:PutItem",
      "dynamodb:UpdateItem",
      "dynamodb:DeleteItem",
      "dynamodb:Query",
      "dynamodb:Scan",
    ]
    resources = [
      "arn:aws:dynamodb:${local.region}:*:table/${var.users_table_name}",
      "arn:aws:dynamodb:${local.region}:*:table/${var.users_table_name}/index/*",
      "arn:aws:dynamodb:${local.region}:*:table/${var.credentials_table_name}",
      "arn:aws:dynamodb:${local.region}:*:table/${var.credentials_table_name}/index/*",
      "arn:aws:dynamodb:${local.region}:*:table/${var.refresh_tokens_table_name}",
      "arn:aws:dynamodb:${local.region}:*:table/${var.refresh_tokens_table_name}/index/*",
      aws_dynamodb_table.email_tokens.arn,
      "${aws_dynamodb_table.email_tokens.arn}/index/*",
      aws_dynamodb_table.challenges.arn,
      "${aws_dynamodb_table.challenges.arn}/index/*",
      aws_dynamodb_table.oauth_devices.arn,
      "${aws_dynamodb_table.oauth_devices.arn}/index/*",
    ]
  }
}

resource "aws_iam_role_policy" "lambda_dynamodb" {
  name   = "dynamodb-access"
  role   = aws_iam_role.lambda_exec.id
  policy = data.aws_iam_policy_document.lambda_dynamodb.json
}

data "aws_iam_policy_document" "lambda_kms" {
  statement {
    effect    = "Allow"
    actions   = ["kms:Sign", "kms:GetPublicKey"]
    resources = [local.kms_key_arn]
  }
}

resource "aws_iam_role_policy" "lambda_kms" {
  name   = "kms-signing"
  role   = aws_iam_role.lambda_exec.id
  policy = data.aws_iam_policy_document.lambda_kms.json
}

# ── ACM Certificate for HTTPS ────────────────────────────────────────────────

resource "aws_acm_certificate" "api" {
  domain_name       = "api.${var.domain_name}"
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

# DNS validation records — only created in primary region
# All regions share the same domain and DNS validation will apply to all regional certs
# Terraform cannot create duplicate Route53 records across regions, so we skip in replicas

resource "aws_route53_record" "acm_validation" {
  for_each = var.is_primary ? {
    for dvo in aws_acm_certificate.api.domain_validation_options :
    dvo.domain => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  } : {}

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = var.route53_zone_id
}

resource "aws_acm_certificate_validation" "api" {
  certificate_arn = aws_acm_certificate.api.arn
  timeouts {
    create = "5m"
  }
  depends_on = [aws_route53_record.acm_validation]
}

# ── HTTP API Gateway ──────────────────────────────────────────────────────────

resource "aws_apigatewayv2_api" "main" {
  name          = "${local.prefix}-api"
  protocol_type = "HTTP"

  cors_configuration {
    allow_origins = ["*"] # tighten in prod
    allow_methods = ["GET", "POST", "PATCH", "DELETE", "OPTIONS"]
    allow_headers = ["content-type", "authorization"]
    max_age       = 300
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.main.id
  name        = "$default"
  auto_deploy = true
}

# ── Custom Domain Name (HTTPS) ────────────────────────────────────────────────

resource "aws_apigatewayv2_domain_name" "api" {
  domain_name = "api.${var.domain_name}"

  domain_name_configuration {
    certificate_arn = aws_acm_certificate.api.arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }

  depends_on = [aws_acm_certificate_validation.api]

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_apigatewayv2_api_mapping" "api" {
  api_id      = aws_apigatewayv2_api.main.id
  domain_name = aws_apigatewayv2_domain_name.api.domain_name
  stage       = aws_apigatewayv2_stage.default.name
}

# Route53 alias record pointing to the API Gateway custom domain
# Created only in primary region to avoid duplicate record errors
resource "aws_route53_record" "api_domain" {
  for_each = var.is_primary ? toset(["enabled"]) : toset([])

  name    = aws_apigatewayv2_domain_name.api.domain_name
  type    = "A"
  zone_id = var.route53_zone_id

  alias {
    name                   = aws_apigatewayv2_domain_name.api.domain_name_configuration[0].target_domain_name
    zone_id                = aws_apigatewayv2_domain_name.api.domain_name_configuration[0].hosted_zone_id
    evaluate_target_health = false
  }
}

# ── auth-lambda ───────────────────────────────────────────────────────────────

# TODO: aws_lambda_function.auth — zip/image source wired to S3 artifact bucket
# TODO: aws_lambda_permission.auth_apigw — allow API GW to invoke
# TODO: aws_apigatewayv2_integration.auth — Lambda proxy integration
# TODO: aws_apigatewayv2_route entries for /auth/*, /oauth/*, /.well-known/*, /register, /activate

# ── users-lambda ─────────────────────────────────────────────────────────────

# TODO: aws_lambda_function.users — zip/image source wired to S3 artifact bucket
# TODO: aws_lambda_permission.users_apigw
# TODO: aws_apigatewayv2_integration.users
# TODO: aws_apigatewayv2_route entries for /me, /admin/*
