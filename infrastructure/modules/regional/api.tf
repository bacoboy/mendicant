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
}

# ── IAM execution role (shared by both Lambdas) ───────────────────────────────

resource "aws_iam_role" "lambda_exec" {
  name = "${local.prefix}-lambda-exec"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "lambda_basic" {
  role       = aws_iam_role.lambda_exec.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.lambda_exec.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "dynamodb:GetItem",
        "dynamodb:PutItem",
        "dynamodb:UpdateItem",
        "dynamodb:DeleteItem",
        "dynamodb:Query",
        "dynamodb:Scan",
      ]
      Resource = [
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
    }]
  })
}

resource "aws_iam_role_policy" "lambda_kms" {
  name = "kms-signing"
  role = aws_iam_role.lambda_exec.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["kms:Sign", "kms:GetPublicKey"]
      Resource = local.kms_key_arn
    }]
  })
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
