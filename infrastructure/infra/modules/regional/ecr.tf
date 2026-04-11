# ECR repositories for Lambda container images.
# One repo per Lambda function, per region. CI pushes directly to each region.

locals {
  ecr_lifecycle_policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Expire untagged images after 1 day"
        selection = {
          tagStatus   = "untagged"
          countType   = "sinceImagePushed"
          countUnit   = "days"
          countNumber = 1
        }
        action = { type = "expire" }
      },
      {
        rulePriority = 2
        description  = "Keep last 10 tagged images"
        selection = {
          tagStatus     = "tagged"
          tagPrefixList = ["sha-"]
          countType     = "imageCountMoreThan"
          countNumber   = 10
        }
        action = { type = "expire" }
      },
    ]
  })
}

resource "aws_ecr_repository" "auth_lambda" {
  name                 = "${local.prefix}-auth-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_ecr_lifecycle_policy" "auth_lambda" {
  repository = aws_ecr_repository.auth_lambda.name
  policy     = local.ecr_lifecycle_policy
}

resource "aws_ecr_repository" "users_lambda" {
  name                 = "${local.prefix}-users-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_ecr_lifecycle_policy" "users_lambda" {
  repository = aws_ecr_repository.users_lambda.name
  policy     = local.ecr_lifecycle_policy
}
