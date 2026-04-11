# HTTP API Gateway, custom domain, and DNS records for this region.
# Lambda integrations and routes live in infrastructure/app (they change with deployments).

resource "aws_apigatewayv2_api" "main" {
  name          = "${local.prefix}-api"
  protocol_type = "HTTP"

  cors_configuration {
    allow_origins = ["https://${var.domain_name}"]
    allow_methods = ["GET", "POST", "PATCH", "DELETE", "OPTIONS"]
    allow_headers = ["content-type", "authorization"]
    max_age       = 300
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.main.id
  name        = "$default"
  auto_deploy = true
}

# ── ACM Certificate ───────────────────────────────────────────────────────────

resource "aws_acm_certificate" "api" {
  domain_name       = "api.${var.domain_name}"
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

# DNS validation records only in the primary region — all regional certs share
# the same domain so the validation record is identical across regions.
resource "aws_route53_record" "acm_validation" {
  for_each = var.is_primary ? {
    for dvo in aws_acm_certificate.api.domain_validation_options :
    dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  } : {}

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = var.route53_zone_id
}

resource "aws_acm_certificate_validation" "api" {
  certificate_arn = aws_acm_certificate.api.arn
  timeouts {
    create = "5m"
  }
  depends_on = [aws_route53_record.acm_validation]
}

# ── Custom Domain ─────────────────────────────────────────────────────────────

resource "aws_apigatewayv2_domain_name" "api" {
  domain_name = "api.${var.domain_name}"

  domain_name_configuration {
    certificate_arn = aws_acm_certificate.api.arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
    ip_address_type = "dualstack"
  }

  depends_on = [aws_acm_certificate_validation.api]

  tags = {
    app         = var.app_name
    environment = var.environment
    region      = local.region
  }
}

resource "aws_apigatewayv2_api_mapping" "api" {
  api_id      = aws_apigatewayv2_api.main.id
  domain_name = aws_apigatewayv2_domain_name.api.domain_name
  stage       = aws_apigatewayv2_stage.default.name
}

# Latency-based A + AAAA alias records — geo-closest region wins.
resource "aws_route53_record" "api_domain" {
  for_each = toset(["A", "AAAA"])

  name           = aws_apigatewayv2_domain_name.api.domain_name
  type           = each.key
  zone_id        = var.route53_zone_id
  set_identifier = "${local.region}-${each.key}"

  alias {
    name                   = aws_apigatewayv2_domain_name.api.domain_name_configuration[0].target_domain_name
    zone_id                = aws_apigatewayv2_domain_name.api.domain_name_configuration[0].hosted_zone_id
    evaluate_target_health = false
  }

  latency_routing_policy {
    region = local.region
  }
}
