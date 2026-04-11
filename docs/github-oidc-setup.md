# GitHub Actions OIDC Setup for AWS

One-time bootstrap to allow GitHub Actions to push Docker images to ECR in each region without storing long-lived AWS credentials.

## Prerequisites

- AWS account ID: `054297229654`
- GitHub org/repo: update the trust policy below with your actual `org/repo`
- Regions: `us-east-2`, `us-west-2`

---

## Step 1 — Create the OIDC Provider (account-level, one time)

Run this once per AWS account. If you already have a GitHub OIDC provider, skip this.

```bash
aws iam create-open-id-connect-provider \
  --url https://token.actions.githubusercontent.com \
  --client-id-list sts.amazonaws.com \
  --thumbprint-list 6938fd4d98bab03faadb97b34396831e3780aea1 \
  --region us-east-2
```

Verify it exists:

```bash
aws iam list-open-id-connect-providers
```

---

## Step 2 — Create the IAM Role

Create a file `github-actions-trust.json`:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Principal": {
        "Federated": "arn:aws:iam::054297229654:oidc-provider/token.actions.githubusercontent.com"
      },
      "Action": "sts:AssumeRoleWithWebIdentity",
      "Condition": {
        "StringEquals": {
          "token.actions.githubusercontent.com:aud": "sts.amazonaws.com"
        },
        "StringLike": {
          "token.actions.githubusercontent.com:sub": "repo:YOUR_ORG/mendicant:*"
        }
      }
    }
  ]
}
```

Replace `YOUR_ORG/mendicant` with your actual GitHub org and repo name (e.g. `acme/mendicant`).

To restrict to only the `main` branch (recommended for production pushes):

```json
"token.actions.githubusercontent.com:sub": "repo:YOUR_ORG/mendicant:ref:refs/heads/main"
```

Create the role:

```bash
aws iam create-role \
  --role-name mendicant-github-actions \
  --assume-role-policy-document file://github-actions-trust.json \
  --region us-east-2
```

---

## Step 3 — Attach ECR Permissions

The role needs to authenticate with ECR and push images to repositories in both regions. ECR `GetAuthorizationToken` is global (no resource restriction), but push permissions are per-repository.

Create `github-actions-ecr-policy.json`:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "ECRAuth",
      "Effect": "Allow",
      "Action": "ecr:GetAuthorizationToken",
      "Resource": "*"
    },
    {
      "Sid": "ECRPush",
      "Effect": "Allow",
      "Action": [
        "ecr:BatchCheckLayerAvailability",
        "ecr:CompleteLayerUpload",
        "ecr:InitiateLayerUpload",
        "ecr:PutImage",
        "ecr:UploadLayerPart",
        "ecr:BatchGetImage",
        "ecr:GetDownloadUrlForLayer"
      ],
      "Resource": [
        "arn:aws:ecr:us-east-2:054297229654:repository/mendicant-auth-lambda",
        "arn:aws:ecr:us-east-2:054297229654:repository/mendicant-users-lambda",
        "arn:aws:ecr:us-west-2:054297229654:repository/mendicant-auth-lambda",
        "arn:aws:ecr:us-west-2:054297229654:repository/mendicant-users-lambda"
      ]
    }
  ]
}
```

Attach the policy:

```bash
aws iam put-role-policy \
  --role-name mendicant-github-actions \
  --policy-name ecr-push \
  --policy-document file://github-actions-ecr-policy.json
```

Note the role ARN for the next step:

```bash
aws iam get-role --role-name mendicant-github-actions --query 'Role.Arn' --output text
# arn:aws:iam::054297229654:role/mendicant-github-actions
```

---

## Step 4 — Add GitHub Repository Variable

In your GitHub repo: **Settings → Secrets and variables → Actions → Variables** (not Secrets — this value is not sensitive):

| Name | Value |
|---|---|
| `AWS_ROLE_ARN` | `arn:aws:iam::054297229654:role/mendicant-github-actions` |

No other secrets are needed. OIDC handles authentication automatically.

---

## Step 5 — Verify the workflow

The `.github/workflows/build.yml` workflow (to be added) will use this role via:

```yaml
permissions:
  id-token: write   # required for OIDC
  contents: read

- uses: aws-actions/configure-aws-credentials@v4
  with:
    role-to-assume: ${{ vars.AWS_ROLE_ARN }}
    aws-region: us-east-2
```

Push to `main` and confirm the Actions run authenticates successfully and images appear in both regional ECR repos.

---

## Troubleshooting

**`AccessDenied` on `AssumeRoleWithWebIdentity`** — the `sub` claim in the trust policy doesn't match. Check the exact `org/repo` string and branch condition. You can see the actual claim value in the Actions run logs by adding a debug step.

**`ecr:GetAuthorizationToken` denied** — this action does not support resource restrictions, so `"Resource": "*"` is required. Verify the policy was attached to the correct role.

**Images not appearing in us-west-2** — the workflow pushes to each region independently. Check the matrix job for that region in the Actions UI.
