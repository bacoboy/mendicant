output "api_domain" {
  value = "api.mendicant.io"
}

output "auth_lambda_ecr_url_us_east_2" {
  value = module.regional_us_east_2.auth_lambda_ecr_url
}

output "auth_lambda_ecr_url_us_west_2" {
  value = module.regional_us_west_2.auth_lambda_ecr_url
}

output "users_lambda_ecr_url_us_east_2" {
  value = module.regional_us_east_2.users_lambda_ecr_url
}

output "users_lambda_ecr_url_us_west_2" {
  value = module.regional_us_west_2.users_lambda_ecr_url
}
