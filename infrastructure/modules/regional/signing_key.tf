# Replica of the primary KMS multi-region key for this region.
# Lambdas sign and verify JWTs using their local replica, avoiding cross-region
# KMS calls on every request.
#
# Not created in the primary region (us-east-2) — the primary key already
# lives there and is referenced directly via kms_signing_key_arn.

resource "aws_kms_replica_key" "jwt_signing" {
  count = var.is_primary ? 0 : 1

  description     = "${local.prefix} JWT signing key replica (${local.region})"
  primary_key_arn = var.kms_signing_key_arn
  enabled         = true

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_kms_alias" "jwt_signing" {
  count = var.is_primary ? 0 : 1

  name          = "alias/${local.prefix}-jwt-signing"
  target_key_id = aws_kms_replica_key.jwt_signing[0].key_id
}

locals {
  # In the primary region, use the primary key ARN directly.
  # In replica regions, use the local replica.
  kms_key_arn = var.is_primary ? var.kms_signing_key_arn : aws_kms_replica_key.jwt_signing[0].arn
}
