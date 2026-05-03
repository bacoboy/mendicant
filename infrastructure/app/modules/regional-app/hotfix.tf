# Hotfix Lambda functions — zip-type, same config as prod.
# Terraform manages configuration (env vars, role, memory, timeout).
# Code is deployed directly via: cargo lambda deploy --function-name <name>
# Traffic is swapped via: scripts/hotfix-swap.sh activate|deactivate

# Minimal placeholder zip for initial creation.
# cargo lambda deploy replaces this on first dev deploy.
data "archive_file" "hotfix_placeholder" {
  type        = "zip"
  output_path = "/tmp/mendicant-hotfix-placeholder.zip"
  source {
    content  = "placeholder"
    filename = "bootstrap"
  }
}

# ── auth-lambda hotfix ─────────────────────────────────────────────────────────

resource "aws_lambda_function" "auth_hotfix" {
  function_name = "${local.prefix}-auth-hotfix-${local.region}"
  role          = local.exec_role_arn
  package_type  = "Zip"
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  filename      = data.archive_file.hotfix_placeholder.output_path
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

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_permission" "auth_hotfix_apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.auth_hotfix.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:aws:execute-api:${local.region}:${data.aws_caller_identity.current.account_id}:${local.api_gw_id}/*/*"
}

# ── users-lambda hotfix ────────────────────────────────────────────────────────

resource "aws_lambda_function" "users_hotfix" {
  function_name = "${local.prefix}-users-hotfix-${local.region}"
  role          = local.exec_role_arn
  package_type  = "Zip"
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  filename      = data.archive_file.hotfix_placeholder.output_path
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

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_permission" "users_hotfix_apigw" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.users_hotfix.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "arn:aws:execute-api:${local.region}:${data.aws_caller_identity.current.account_id}:${local.api_gw_id}/*/*"
}
