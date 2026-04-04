# Multi-Region KMS key used to sign JWTs (RS256).
# The primary key lives in us-east-2. Each regional module creates a replica
# in its own region so Lambdas can sign and verify locally without cross-region
# KMS calls. The same public key is valid everywhere.

resource "aws_kms_key" "jwt_signing" {
  description              = "${local.prefix} JWT signing key (RS256)"
  key_usage                = "SIGN_VERIFY"
  customer_master_key_spec = "RSA_4096"
  multi_region             = true
  enable_key_rotation      = false # rotation not supported for asymmetric keys

  tags = {
    app         = var.app_name
    environment = var.environment
  }
}

resource "aws_kms_alias" "jwt_signing" {
  name          = "alias/${local.prefix}-jwt-signing"
  target_key_id = aws_kms_key.jwt_signing.key_id
}
