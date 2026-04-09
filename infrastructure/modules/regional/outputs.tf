output "api_endpoint" {
  description = "HTTP API Gateway invoke URL for this region."
  value       = aws_apigatewayv2_stage.default.invoke_url
}

output "kms_key_arn" {
  description = "KMS key ARN used for JWT signing in this region (primary or replica)."
  value       = local.kms_key_arn
}

output "email_tokens_table_name" {
  value = aws_dynamodb_table.email_tokens.name
}

output "challenges_table_name" {
  value = aws_dynamodb_table.challenges.name
}

output "oauth_devices_table_name" {
  value = aws_dynamodb_table.oauth_devices.name
}
