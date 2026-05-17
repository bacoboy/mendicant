variable "app_name" {
  type = string
}

variable "environment" {
  type = string
}

variable "auth_image_tag" {
  description = "ECR image tag for auth-lambda (e.g. sha-abc1234)."
  type        = string
}

variable "user_image_tag" {
  description = "ECR image tag for user-lambda (e.g. sha-abc1234)."
  type        = string
}

variable "admin_image_tag" {
  description = "ECR image tag for admin-lambda (e.g. sha-abc1234)."
  type        = string
}

variable "rp_id" {
  description = "WebAuthn Relying Party ID (the domain, e.g. mendicant.io)."
  type        = string
}

variable "rp_origins" {
  description = "Comma-separated list of allowed WebAuthn origins (e.g. \"https://api.mendicant.io,https://beta.mendicant.io\")."
  type        = string
}

variable "base_url" {
  description = "Base URL for constructing links in emails and OAuth redirects."
  type        = string
}

variable "domain_name" {
  description = "Root domain name (e.g. mendicant.io). Used to derive SES from address."
  type        = string
}
