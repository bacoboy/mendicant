variable "image_tag" {
  description = "ECR image tag to deploy (e.g. sha-abc1234). Printed by the build workflow after each push."
  type        = string
}