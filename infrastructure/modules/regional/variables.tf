variable "app_name" {
  type = string
}

variable "environment" {
  type = string
}

variable "is_primary" {
  description = "True for the global/primary region (us-east-2). Controls which resources are primary vs replica."
  type        = bool
  default     = false
}

variable "kms_signing_key_arn" {
  description = "ARN of the primary KMS multi-region key from the global module."
  type        = string
}

variable "users_table_name" {
  type = string
}

variable "credentials_table_name" {
  type = string
}

variable "refresh_tokens_table_name" {
  type = string
}

variable "domain_name" {
  description = "Root domain name (from global module)"
  type        = string
}

variable "route53_zone_id" {
  description = "Route53 hosted zone ID for mendicant.io (from global module)"
  type        = string
}
