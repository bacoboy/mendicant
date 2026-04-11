# IAM execution role shared by all Lambda functions in this region.

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
  name               = "${local.prefix}-lambda-exec-${local.region}"
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
