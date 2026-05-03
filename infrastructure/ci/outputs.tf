output "github_actions_role_arn" {
  description = "Set as AWS_ROLE_ARN in GitHub repository variables (Settings → Secrets and variables → Variables)"
  value       = aws_iam_role.github_actions.arn
}
