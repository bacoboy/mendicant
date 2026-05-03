#!/usr/bin/env bash
# Swap API Gateway integrations between prod (ECR image) and hotfix (zip) Lambdas.
# Traffic continues flowing through the same API GW URL — no client changes needed.
#
# Usage:
#   ./scripts/hotfix-swap.sh activate   [region]   # route traffic to hotfix
#   ./scripts/hotfix-swap.sh deactivate [region]   # restore to prod
#
# Workflow:
#   1. ./scripts/dev-deploy.sh           # build + push code to hotfix function
#   2. ./scripts/hotfix-swap.sh activate # cut traffic over
#   3. Test at the real production URL
#   4. ./scripts/hotfix-swap.sh deactivate  # restore when done

set -euo pipefail

ACTION=${1:-}
REGION=${2:-us-east-2}
PREFIX=mendicant-prod
ACCOUNT_ID=054297229654

if [[ "${ACTION}" != "activate" && "${ACTION}" != "deactivate" ]]; then
    echo "Usage: $0 activate|deactivate [region]"
    exit 1
fi

# Resolve function ARNs
prod_auth_arn="arn:aws:lambda:${REGION}:${ACCOUNT_ID}:function:${PREFIX}-auth-${REGION}"
prod_users_arn="arn:aws:lambda:${REGION}:${ACCOUNT_ID}:function:${PREFIX}-users-${REGION}"
hotfix_auth_arn="arn:aws:lambda:${REGION}:${ACCOUNT_ID}:function:${PREFIX}-auth-hotfix-${REGION}"
hotfix_users_arn="arn:aws:lambda:${REGION}:${ACCOUNT_ID}:function:${PREFIX}-users-hotfix-${REGION}"

arn_to_integration_uri() {
    echo "arn:aws:apigateway:${REGION}:lambda:path/2015-03-31/functions/${1}/invocations"
}

# Find the API GW
echo "==> Finding API Gateway..."
api_id=$(aws apigatewayv2 get-apis \
    --region "${REGION}" \
    --query "Items[?Name=='${PREFIX}-api'].ApiId" \
    --output text)

if [[ -z "${api_id}" ]]; then
    echo "Error: could not find API Gateway named '${PREFIX}-api' in ${REGION}"
    exit 1
fi
echo "    API Gateway: ${api_id}"

# Find integration IDs by matching current integration URI
get_integration_id() {
    local function_arn=$1
    local uri
    uri=$(arn_to_integration_uri "${function_arn}")
    aws apigatewayv2 get-integrations \
        --region "${REGION}" \
        --api-id "${api_id}" \
        --query "Items[?IntegrationUri=='${uri}'].IntegrationId" \
        --output text
}

update_integration() {
    local integration_id=$1
    local target_arn=$2
    local uri
    uri=$(arn_to_integration_uri "${target_arn}")
    aws apigatewayv2 update-integration \
        --region "${REGION}" \
        --api-id "${api_id}" \
        --integration-id "${integration_id}" \
        --integration-uri "${uri}" \
        --output text \
        --query 'IntegrationId' > /dev/null
}

if [[ "${ACTION}" == "activate" ]]; then
    echo "==> Switching to hotfix Lambdas..."

    auth_integration=$(get_integration_id "${prod_auth_arn}")
    users_integration=$(get_integration_id "${prod_users_arn}")

    if [[ -z "${auth_integration}" || -z "${users_integration}" ]]; then
        echo "Error: could not find prod integrations — are they already swapped?"
        exit 1
    fi

    update_integration "${auth_integration}" "${hotfix_auth_arn}"
    echo "    ✓ auth  → hotfix"

    update_integration "${users_integration}" "${hotfix_users_arn}"
    echo "    ✓ users → hotfix"

    echo ""
    echo "Hotfix active. Test at https://api.mendicant.io"
    echo "Restore when done: $0 deactivate ${REGION}"

else
    echo "==> Restoring prod Lambdas..."

    auth_integration=$(get_integration_id "${hotfix_auth_arn}")
    users_integration=$(get_integration_id "${hotfix_users_arn}")

    if [[ -z "${auth_integration}" || -z "${users_integration}" ]]; then
        echo "Error: could not find hotfix integrations — are they already restored?"
        exit 1
    fi

    update_integration "${auth_integration}" "${prod_auth_arn}"
    echo "    ✓ auth  → prod"

    update_integration "${users_integration}" "${prod_users_arn}"
    echo "    ✓ users → prod"

    echo ""
    echo "Prod restored."
fi
