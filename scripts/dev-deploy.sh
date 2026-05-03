#!/usr/bin/env bash
# Hotfix deploy: build locally with cargo lambda → push to hotfix Lambdas in both regions.
# No Docker, no ECR, no Terraform. Fast iteration against real AWS.
#
# Usage:
#   ./scripts/dev-deploy.sh
#
# Then use hotfix-swap.sh to route traffic to/from the hotfix functions.

set -euo pipefail

ulimit -n 8192

REGIONS=(us-east-2 us-west-2)
LAMBDAS=(auth users)
PREFIX=mendicant-prod

for name in "${LAMBDAS[@]}"; do
    echo "==> Building ${name}-lambda (arm64)..."
    cargo lambda build --release -p "${name}-lambda" --arm64
done

for name in "${LAMBDAS[@]}"; do
    for region in "${REGIONS[@]}"; do
        function_name="${PREFIX}-${name}-hotfix-${region}"
        echo "==> Deploying ${name}-lambda → ${function_name}..."
        cargo lambda deploy \
            --binary-name "${name}-lambda" \
            --region "${region}" \
            "${function_name}"
        echo "    ✓ done"
    done
done

echo ""
echo "Both lambdas deployed to both regions."
echo "Run ./scripts/hotfix-swap.sh activate to route traffic to the hotfix functions."
