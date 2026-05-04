# Postmark DNS records — used for transactional email testing.
# These are separate from the SES identity; Postmark handles sending
# while we validate the email verification flow before enabling SES
# production access.

# Postmark-issued DKIM key for mendicant.io.
resource "aws_route53_record" "postmark_dkim" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "20260504002701pm._domainkey.${local.domain_name}"
  type    = "TXT"
  ttl     = 300
  records = ["k=rsa;p=MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQCnmZJlHfTkqadyU6s7jKhje5YC79742A2S76lu99URUzxrnvA3RueCc6hjfWxAPcNkgrodgpljv6eoFyW7bHLnglTYMBPoXDpYzlcSwvIY0UPWPbt+MPBNxICouBfH+oqTJx3kr+oBDkdzAxXNtElGmWRj/1Ar9Dg+0KvIdtDaqwIDAQAB"]
}

# Return-Path domain for Postmark bounce handling.
resource "aws_route53_record" "postmark_return_path" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "pm-bounces.${local.domain_name}"
  type    = "CNAME"
  ttl     = 300
  records = ["pm.mtasv.net"]
}
