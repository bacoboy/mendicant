# KMS replica of the primary multi-region key.
# Not created in us-east-2 — the primary key already lives there.

resource "aws_kms_replica_key" "jwt_signing" {
  for_each = var.is_primary ? toset([]) : toset(["replica"])

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
  for_each = var.is_primary ? toset([]) : toset(["replica"])

  name          = "alias/${local.prefix}-jwt-signing"
  target_key_id = aws_kms_replica_key.jwt_signing["replica"].key_id
}

locals {
  kms_key_arn = var.is_primary ? var.kms_signing_key_arn : aws_kms_replica_key.jwt_signing["replica"].arn
}
