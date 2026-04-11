output "api_endpoint" {
  value = aws_apigatewayv2_stage.default.invoke_url
}

output "auth_lambda_ecr_url" {
  value = aws_ecr_repository.auth_lambda.repository_url
}

output "users_lambda_ecr_url" {
  value = aws_ecr_repository.users_lambda.repository_url
}
