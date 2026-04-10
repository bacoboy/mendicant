# CloudFront and Route53 are provisioned here when a domain_name is provided.
# In dev with no domain, these resources are skipped.

data "aws_route53_zone" "main" {
  name = var.domain_name
}

# TODO: CloudFront distribution (frontend SPA origin from S3, API origin from regional API GW)
# TODO: Route53 latency-based alias records per region pointing to regional API Gateway endpoints
# TODO: ACM certificate in us-east-1 for CloudFront (separate from regional API certs)
