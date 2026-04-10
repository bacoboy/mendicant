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

output "route53_zone_id" {
  description = "Route53 hosted zone ID for mendicant.io"
  value       = data.aws_route53_zone.main.zone_id
}

output "domain_name" {
  description = "Root domain name"
  value       = var.domain_name
}
