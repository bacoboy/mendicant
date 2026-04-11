# Lambda functions, API Gateway integrations, and routes.
#
# Route ownership:
#   auth-lambda  — all HTML pages, WebAuthn flows, OAuth device flow, JWKS, static assets
#   users-lambda — profile API (PATCH /me), admin user management (/admin/users/*)
#
# The $default catch-all routes everything unmatched to auth-lambda.
# Specific routes for users-lambda take precedence (API GW evaluates most-specific first).

# ── auth-lambda ───────────────────────────────────────────────────────────────

resource "aws_lambda_function" "auth" {
  function_name = "${local.prefix}-auth-${local.region}"
  role          = local.exec_role_arn
  package_type  = "Image"
  image_uri     = "${local.ecr_auth}:${var.image_tag}"
  architectures = ["arm64"]
  timeout       = 30
  memory_size   = 256

  environment {
    variables = local.lambda_env
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_lambda_permission" "auth_apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.auth.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:aws:execute-api:${local.region}:${data.aws_caller_identity.current.account_id}:${local.api_gw_id}/*/*"
}

resource "aws_apigatewayv2_integration" "auth" {
  api_id                 = local.api_gw_id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.auth.invoke_arn
  payload_format_version = "2.0"
}

# Catch-all — auth-lambda handles everything not matched by a more specific route.
resource "aws_apigatewayv2_route" "auth_default" {
  api_id    = local.api_gw_id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.auth.id}"
}

# ── users-lambda ──────────────────────────────────────────────────────────────

resource "aws_lambda_function" "users" {
  function_name = "${local.prefix}-users-${local.region}"
  role          = local.exec_role_arn
  package_type  = "Image"
  image_uri     = "${local.ecr_users}:${var.image_tag}"
  architectures = ["arm64"]
  timeout       = 30
  memory_size   = 256

  environment {
    variables = local.lambda_env
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_lambda_permission" "users_apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.users.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:aws:execute-api:${local.region}:${data.aws_caller_identity.current.account_id}:${local.api_gw_id}/*/*"
}

resource "aws_apigatewayv2_integration" "users" {
  api_id                 = local.api_gw_id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.users.invoke_arn
  payload_format_version = "2.0"
}

# Explicit routes for users-lambda. All other traffic falls through to $default (auth-lambda).
resource "aws_apigatewayv2_route" "users_routes" {
  for_each = toset([
    "PATCH /me",
    "GET /admin/users",
    "GET /admin/users/{id}",
    "PATCH /admin/users/{id}",
    "DELETE /admin/users/{id}",
  ])

  api_id    = local.api_gw_id
  route_key = each.key
  target    = "integrations/${aws_apigatewayv2_integration.users.id}"
}
