variable "app_name" {
  type = string
}

variable "environment" {
  type = string
}

variable "is_primary" {
  description = "True for us-east-2. Controls whether a KMS replica is created (primary already has the key)."
  type        = bool
  default     = false
}

variable "kms_signing_key_arn" {
  description = "ARN of the primary KMS multi-region key."
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
  type = string
}

variable "route53_zone_id" {
  type = string
}

variable "ses_identity_arn" {
  description = "ARN of the SES domain identity in us-east-2. Used to scope the Lambda send permission."
  type        = string
}
