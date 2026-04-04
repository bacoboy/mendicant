variable "app_name" {
  description = "Application name, used as a prefix for all resource names."
  type        = string
}

variable "environment" {
  description = "Deployment environment (dev, prod)."
  type        = string
}

variable "replica_regions" {
  description = "AWS regions that will receive DynamoDB Global Table replicas."
  type        = list(string)
}

variable "domain_name" {
  description = "Root domain name (e.g. example.com). Used for Route53 and CloudFront."
  type        = string
  default     = ""
}
