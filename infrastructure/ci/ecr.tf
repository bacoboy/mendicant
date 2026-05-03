# ECR repositories for Lambda container images.
# One repo per Lambda function, per region. CI pushes to both regions in parallel.

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
        description  = "Keep last 10 release images"
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

# ── us-east-2 ─────────────────────────────────────────────────────────────────

resource "aws_ecr_repository" "auth_lambda_us_east_2" {
  provider = aws.us_east_2

  name                 = "${local.prefix}-auth-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = { app = local.app_name, environment = local.environment }
}

resource "aws_ecr_lifecycle_policy" "auth_lambda_us_east_2" {
  provider   = aws.us_east_2
  repository = aws_ecr_repository.auth_lambda_us_east_2.name
  policy     = local.ecr_lifecycle_policy
}

resource "aws_ecr_repository" "users_lambda_us_east_2" {
  provider = aws.us_east_2

  name                 = "${local.prefix}-users-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = { app = local.app_name, environment = local.environment }
}

resource "aws_ecr_lifecycle_policy" "users_lambda_us_east_2" {
  provider   = aws.us_east_2
  repository = aws_ecr_repository.users_lambda_us_east_2.name
  policy     = local.ecr_lifecycle_policy
}

# ── us-west-2 ─────────────────────────────────────────────────────────────────

resource "aws_ecr_repository" "auth_lambda_us_west_2" {
  provider = aws.us_west_2

  name                 = "${local.prefix}-auth-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = { app = local.app_name, environment = local.environment }
}

resource "aws_ecr_lifecycle_policy" "auth_lambda_us_west_2" {
  provider   = aws.us_west_2
  repository = aws_ecr_repository.auth_lambda_us_west_2.name
  policy     = local.ecr_lifecycle_policy
}

resource "aws_ecr_repository" "users_lambda_us_west_2" {
  provider = aws.us_west_2

  name                 = "${local.prefix}-users-lambda"
  image_tag_mutability = "IMMUTABLE"

  image_scanning_configuration {
    scan_on_push = true
  }

  tags = { app = local.app_name, environment = local.environment }
}

resource "aws_ecr_lifecycle_policy" "users_lambda_us_west_2" {
  provider   = aws.us_west_2
  repository = aws_ecr_repository.users_lambda_us_west_2.name
  policy     = local.ecr_lifecycle_policy
}
