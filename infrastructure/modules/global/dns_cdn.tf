# CloudFront and Route53 are provisioned here when a domain_name is provided.
# In dev with no domain, these resources are skipped.

# TODO: CloudFront distribution (frontend SPA origin from S3, API origin from regional API GW)
# TODO: Route53 hosted zone lookup + latency-based alias records per region
# TODO: ACM certificate (must be in us-east-1 for CloudFront)
