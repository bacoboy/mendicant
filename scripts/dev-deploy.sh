#!/usr/bin/env bash
# Hotfix deploy: build locally with cargo lambda → push directly to hotfix Lambda.
# No Docker, no ECR, no Terraform. Fast iteration against real AWS.
#
# Usage:
#   ./scripts/dev-deploy.sh [auth|users|both] [region]
#
# Defaults: both lambdas, us-east-2
#
# Test via the function URLs printed by: cd infrastructure/app && terraform output
# When ready to promote: push to main and let CI build the real image.

set -euo pipefail

LAMBDA=${1:-both}
REGION=${2:-us-east-2}
PREFIX=mendicant-prod

deploy() {
    local name=$1
    local function_name="${PREFIX}-${name}-hotfix-${REGION}"

    echo "==> Building ${name}-lambda (arm64)..."
    cargo lambda build --release -p "${name}-lambda" --arm64

    echo "==> Deploying to ${function_name}..."
    cargo lambda deploy \
        --binary-name "${name}-lambda" \
        --function-name "${function_name}" \
        --region "${REGION}" \
        --no-build

    echo "    ✓ ${name}-lambda hotfix live"
}

case "${LAMBDA}" in
    auth)  deploy auth ;;
    users) deploy users ;;
    both)  deploy auth; deploy users ;;
    *)     echo "Usage: $0 [auth|users|both] [region]"; exit 1 ;;
esac

echo ""
echo "Run ./scripts/hotfix-swap.sh activate to route traffic to the hotfix function."
