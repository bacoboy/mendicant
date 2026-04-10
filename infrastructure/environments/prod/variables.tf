variable "domain_name" {
  description = "Root domain name for Route53 and CloudFront (e.g. example.com)."
  type        = string
  default     = "mendicant.io"
}
