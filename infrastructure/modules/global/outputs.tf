output "users_table_name" {
  value = aws_dynamodb_table.users.name
}

output "users_table_arn" {
  value = aws_dynamodb_table.users.arn
}

output "credentials_table_name" {
  value = aws_dynamodb_table.credentials.name
}

output "credentials_table_arn" {
  value = aws_dynamodb_table.credentials.arn
}

output "refresh_tokens_table_name" {
  value = aws_dynamodb_table.refresh_tokens.name
}

output "refresh_tokens_table_arn" {
  value = aws_dynamodb_table.refresh_tokens.arn
}

output "kms_signing_key_arn" {
  description = "Primary KMS key ARN — pass to each regional module to create replicas."
  value       = aws_kms_key.jwt_signing.arn
}

output "kms_signing_key_id" {
  value = aws_kms_key.jwt_signing.key_id
}
