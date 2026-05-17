variable "auth_image_tag" {
  description = "ECR image tag for auth-lambda (e.g. sha-abc1234). Printed by the build workflow after each push."
  type        = string
}

variable "user_image_tag" {
  description = "ECR image tag for user-lambda (e.g. sha-abc1234). Printed by the build workflow after each push."
  type        = string
}

variable "admin_image_tag" {
  description = "ECR image tag for admin-lambda (e.g. sha-abc1234). Printed by the build workflow after each push."
  type        = string
}
