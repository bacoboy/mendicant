# Lambda functions, API Gateway integrations, and routes.
#
# Route ownership:
#   auth-lambda — establishes JWTs: /auth/*, /oauth/*, /enroll*, /.well-known/*,
#                 public HTML pages, static assets.
#   user-lambda — needs JWT, acts on the current user: /me, /me/*.
#   admin-lambda — admin-only surface (added in phase 2): /admin/*.
#
# Routing strategy: $default catches everything for auth-lambda; explicit
# routes carve out the user-lambda and admin-lambda surfaces (API GW picks
# the most-specific match).

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

# ── user-lambda ───────────────────────────────────────────────────────────────

resource "aws_lambda_function" "user" {
  function_name = "${local.prefix}-user-${local.region}"
  role          = local.exec_role_arn
  package_type  = "Image"
  image_uri     = "${local.ecr_user}:${var.image_tag}"
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

resource "aws_lambda_permission" "user_apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.user.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:aws:execute-api:${local.region}:${data.aws_caller_identity.current.account_id}:${local.api_gw_id}/*/*"
}

resource "aws_apigatewayv2_integration" "user" {
  api_id                 = local.api_gw_id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.user.invoke_arn
  payload_format_version = "2.0"
}

# Explicit routes for user-lambda. Everything else falls through to $default (auth-lambda).
# `ANY /me` covers the bare path; `ANY /me/{proxy+}` covers all child paths.
# `PATCH /admin/users/{id}` is the only admin route handled here; the rest of
# /admin/* stays with auth-lambda until admin-lambda lands in phase 2.
resource "aws_apigatewayv2_route" "user_routes" {
  for_each = toset([
    "ANY /me",
    "ANY /me/{proxy+}",
    "PATCH /admin/users/{id}",
  ])

  api_id    = local.api_gw_id
  route_key = each.key
  target    = "integrations/${aws_apigatewayv2_integration.user.id}"
}

# State migrations for the user-lambda rename (source identifier change only —
# AWS-facing names are unchanged, so these are no-op renames in AWS).
moved {
  from = aws_lambda_function.users
  to   = aws_lambda_function.user
}

moved {
  from = aws_lambda_permission.users_apigw
  to   = aws_lambda_permission.user_apigw
}

moved {
  from = aws_apigatewayv2_integration.users
  to   = aws_apigatewayv2_integration.user
}

moved {
  from = aws_apigatewayv2_route.users_routes
  to   = aws_apigatewayv2_route.user_routes
}
