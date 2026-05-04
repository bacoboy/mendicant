# SES domain identity for mendicant.io.
# Sending region: us-east-2 only — email is not latency-critical.
# SES remains in sandbox mode until production access is requested.

# Sandbox-mode recipient allowlist. Remove once SES production access is granted.
locals {
  ses_verified_emails = [
    "steve@mendicant.io",
  ]
}

resource "aws_sesv2_email_identity" "verified_emails" {
  provider = aws.us_east_2

  for_each       = toset(local.ses_verified_emails)
  email_identity = each.value
}


resource "aws_sesv2_email_identity" "domain" {
  provider       = aws.us_east_2
  email_identity = local.domain_name

  dkim_signing_attributes {
    next_signing_key_length = "RSA_2048_BIT"
  }

  tags = {
    app         = local.app_name
    environment = local.environment
  }
}

# ── DKIM ──────────────────────────────────────────────────────────────────────
# SES Easy DKIM generates exactly 3 CNAME records. Separate resources avoid
# ordering sensitivity that count-based indexing would introduce.

resource "aws_route53_record" "ses_dkim" {
  count = 3

  zone_id = data.aws_route53_zone.main.zone_id
  name    = "${one(aws_sesv2_email_identity.domain.dkim_signing_attributes).tokens[count.index]}._domainkey.${local.domain_name}"
  type    = "CNAME"
  ttl     = 1800
  records = ["${one(aws_sesv2_email_identity.domain.dkim_signing_attributes).tokens[count.index]}.dkim.amazonses.com"]
}

# ── MAIL FROM ─────────────────────────────────────────────────────────────────
# mail.mendicant.io subdomain — keeps MAIL FROM MX off the apex domain.

resource "aws_sesv2_email_identity_mail_from_attributes" "domain" {
  provider       = aws.us_east_2
  email_identity = aws_sesv2_email_identity.domain.email_identity

  behavior_on_mx_failure = "USE_DEFAULT_VALUE"
  mail_from_domain       = "mail.${local.domain_name}"
}

resource "aws_route53_record" "ses_mail_from_mx" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "mail.${local.domain_name}"
  type    = "MX"
  ttl     = 300
  records = ["10 feedback-smtp.us-east-2.amazonses.com"]
}

resource "aws_route53_record" "ses_mail_from_spf" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "mail.${local.domain_name}"
  type    = "TXT"
  ttl     = 300
  records = ["v=spf1 include:amazonses.com ~all"]
}

# ── SPF (apex) ────────────────────────────────────────────────────────────────
# Authorizes both Fastmail and SES to send on behalf of mendicant.io.
# Only one SPF record is allowed per name. Import block below takes ownership
# of the existing record without destroying and recreating it.

import {
  to = aws_route53_record.ses_spf
  id = "${data.aws_route53_zone.main.zone_id}_${local.domain_name}_TXT"
}

resource "aws_route53_record" "ses_spf" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = local.domain_name
  type    = "TXT"
  ttl     = 300
  records = ["v=spf1 include:spf.messagingengine.com include:amazonses.com -all"]
}

# ── DMARC ─────────────────────────────────────────────────────────────────────
# p=none: monitoring-only, no mail rejected.
# TODO: add rua=mailto:<address> once a real mailbox exists for DMARC reports,
# then consider escalating to p=quarantine once SPF/DKIM alignment is confirmed.

import {
  to = aws_route53_record.ses_dmarc
  id = "${data.aws_route53_zone.main.zone_id}__dmarc.${local.domain_name}_TXT"
}

resource "aws_route53_record" "ses_dmarc" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = "_dmarc.${local.domain_name}"
  type    = "TXT"
  ttl     = 300
  records = ["v=DMARC1; p=none;"]
}
