# Import blocks for resources that existed before this Terraform project was
# restructured. Run `terraform plan` to verify no destructive changes, then
# `terraform apply` to absorb them into state. Delete this file afterwards.
#
# Not imported — IAM roles:
#   mendicant-prod-lambda-exec-use2 and mendicant-prod-lambda-exec-usw2 were
#   created with the old short-region naming. Terraform will create new roles
#   with the full region name (us-east-2 / us-west-2). Delete the old roles
#   manually after apply:
#     aws iam delete-role-policy --role-name mendicant-prod-lambda-exec-use2 --policy-name dynamodb-access --profile mendicant
#     aws iam delete-role-policy --role-name mendicant-prod-lambda-exec-use2 --policy-name kms-signing --profile mendicant
#     aws iam detach-role-policy --role-name mendicant-prod-lambda-exec-use2 --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole --profile mendicant
#     aws iam delete-role --role-name mendicant-prod-lambda-exec-use2 --profile mendicant
#     (repeat for usw2)

# ── KMS (primary, us-east-2) ──────────────────────────────────────────────────

import {
  to = aws_kms_key.jwt_signing
  id = "mrk-b516ee4e2bc44b4eaa315b4808259c1d"
}

import {
  to = aws_kms_alias.jwt_signing
  id = "alias/mendicant-prod-jwt-signing"
}

# ── DynamoDB global tables ────────────────────────────────────────────────────

import {
  to = aws_dynamodb_table.users
  id = "mendicant-prod-users"
}

import {
  to = aws_dynamodb_table.credentials
  id = "mendicant-prod-credentials"
}

import {
  to = aws_dynamodb_table.refresh_tokens
  id = "mendicant-prod-refresh-tokens"
}

# ── Regional: us-east-2 ───────────────────────────────────────────────────────

import {
  to = module.regional_us_east_2.aws_dynamodb_table.challenges
  id = "mendicant-prod-challenges"
}

import {
  to = module.regional_us_east_2.aws_dynamodb_table.email_tokens
  id = "mendicant-prod-email-tokens"
}

import {
  to = module.regional_us_east_2.aws_dynamodb_table.oauth_devices
  id = "mendicant-prod-oauth-devices"
}

import {
  to = module.regional_us_east_2.aws_acm_certificate.api
  id = "arn:aws:acm:us-east-2:054297229654:certificate/de2966b0-0346-4a1a-a89b-cdaebf6ea951"
}


import {
  to = module.regional_us_east_2.aws_route53_record.acm_validation["api.mendicant.io"]
  id = "Z2S7UI5ORZ3TWB__0d284b96c35848240928947c108be1d8.api.mendicant.io._CNAME"
}

import {
  to = module.regional_us_east_2.aws_apigatewayv2_api.main
  id = "tkiz5ic1v4"
}

import {
  to = module.regional_us_east_2.aws_apigatewayv2_stage.default
  id = "tkiz5ic1v4/$default"
}

import {
  to = module.regional_us_east_2.aws_apigatewayv2_domain_name.api
  id = "api.mendicant.io"
}

import {
  to = module.regional_us_east_2.aws_apigatewayv2_api_mapping.api
  id = "lbtrvs/api.mendicant.io"
}

import {
  to = module.regional_us_east_2.aws_route53_record.api_domain["A"]
  id = "Z2S7UI5ORZ3TWB_api.mendicant.io_A_us-east-2-A"
}

import {
  to = module.regional_us_east_2.aws_route53_record.api_domain["AAAA"]
  id = "Z2S7UI5ORZ3TWB_api.mendicant.io_AAAA_us-east-2-AAAA"
}

# ── Regional: us-west-2 ───────────────────────────────────────────────────────

import {
  to = module.regional_us_west_2.aws_kms_replica_key.jwt_signing["replica"]
  id = "mrk-b516ee4e2bc44b4eaa315b4808259c1d"
}

import {
  to = module.regional_us_west_2.aws_kms_alias.jwt_signing["replica"]
  id = "alias/mendicant-prod-jwt-signing"
}

import {
  to = module.regional_us_west_2.aws_dynamodb_table.challenges
  id = "mendicant-prod-challenges"
}

import {
  to = module.regional_us_west_2.aws_dynamodb_table.email_tokens
  id = "mendicant-prod-email-tokens"
}

import {
  to = module.regional_us_west_2.aws_dynamodb_table.oauth_devices
  id = "mendicant-prod-oauth-devices"
}

import {
  to = module.regional_us_west_2.aws_acm_certificate.api
  id = "arn:aws:acm:us-west-2:054297229654:certificate/59915814-fc0f-4d41-9d0d-fa75f33dc7a9"
}


import {
  to = module.regional_us_west_2.aws_apigatewayv2_api.main
  id = "wj3o8wf7q4"
}

import {
  to = module.regional_us_west_2.aws_apigatewayv2_stage.default
  id = "wj3o8wf7q4/$default"
}

import {
  to = module.regional_us_west_2.aws_apigatewayv2_domain_name.api
  id = "api.mendicant.io"
}

import {
  to = module.regional_us_west_2.aws_apigatewayv2_api_mapping.api
  id = "0ilkvj/api.mendicant.io"
}

import {
  to = module.regional_us_west_2.aws_route53_record.api_domain["A"]
  id = "Z2S7UI5ORZ3TWB_api.mendicant.io_A_us-west-2-A"
}

import {
  to = module.regional_us_west_2.aws_route53_record.api_domain["AAAA"]
  id = "Z2S7UI5ORZ3TWB_api.mendicant.io_AAAA_us-west-2-AAAA"
}
