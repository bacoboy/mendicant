variable "app_name" {
  type = string
}

variable "environment" {
  type = string
}

variable "image_tag" {
  description = "ECR image tag to deploy (e.g. sha-abc1234)."
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