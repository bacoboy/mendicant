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
REGION=${2:-all}
PREFIX=mendicant-prod
ACCOUNT_ID=054297229654

if [[ "${ACTION}" != "activate" && "${ACTION}" != "deactivate" ]]; then
    echo "Usage: $0 activate|deactivate [us-east-2|us-west-2|all]"
    exit 1
fi

case "${REGION}" in
    us-east-2|us-west-2) REGIONS=("${REGION}") ;;
    all)                 REGIONS=(us-east-2 us-west-2) ;;
    *) echo "Error: unknown region '${REGION}'. Use us-east-2, us-west-2, or all."; exit 1 ;;
esac

arn_to_integration_uri() {
    local region=$1 arn=$2
    echo "arn:aws:apigateway:${region}:lambda:path/2015-03-31/functions/${arn}/invocations"
}

get_integration_id() {
    local region=$1 api_id=$2 function_arn=$3
    local uri
    uri=$(arn_to_integration_uri "${region}" "${function_arn}")
    aws apigatewayv2 get-integrations \
        --region "${region}" \
        --api-id "${api_id}" \
        --query "Items[?IntegrationUri=='${uri}'].IntegrationId" \
        --output text
}

update_integration() {
    local region=$1 api_id=$2 integration_id=$3 target_arn=$4
    local uri
    uri=$(arn_to_integration_uri "${region}" "${target_arn}")
    aws apigatewayv2 update-integration \
        --region "${region}" \
        --api-id "${api_id}" \
        --integration-id "${integration_id}" \
        --integration-uri "${uri}" \
        --output text \
        --query 'IntegrationId' > /dev/null
}

swap_region() {
    local region=$1

    local prod_auth_arn="arn:aws:lambda:${region}:${ACCOUNT_ID}:function:${PREFIX}-auth-${region}"
    local prod_users_arn="arn:aws:lambda:${region}:${ACCOUNT_ID}:function:${PREFIX}-users-${region}"
    local hotfix_auth_arn="arn:aws:lambda:${region}:${ACCOUNT_ID}:function:${PREFIX}-auth-hotfix-${region}"
    local hotfix_users_arn="arn:aws:lambda:${region}:${ACCOUNT_ID}:function:${PREFIX}-users-hotfix-${region}"

    echo "==> [${region}] Finding API Gateway..."
    local api_id
    api_id=$(aws apigatewayv2 get-apis \
        --region "${region}" \
        --query "Items[?Name=='${PREFIX}-api'].ApiId" \
        --output text)

    if [[ -z "${api_id}" ]]; then
        echo "Error: could not find API Gateway named '${PREFIX}-api' in ${region}"
        exit 1
    fi
    echo "    API Gateway: ${api_id}"

    if [[ "${ACTION}" == "activate" ]]; then
        echo "==> [${region}] Switching to hotfix Lambdas..."

        local auth_integration users_integration
        auth_integration=$(get_integration_id "${region}" "${api_id}" "${prod_auth_arn}")
        users_integration=$(get_integration_id "${region}" "${api_id}" "${prod_users_arn}")

        if [[ -z "${auth_integration}" || -z "${users_integration}" ]]; then
            echo "Error: could not find prod integrations in ${region} — are they already swapped?"
            exit 1
        fi

        update_integration "${region}" "${api_id}" "${auth_integration}" "${hotfix_auth_arn}"
        echo "    ✓ auth  → hotfix"

        update_integration "${region}" "${api_id}" "${users_integration}" "${hotfix_users_arn}"
        echo "    ✓ users → hotfix"

    else
        echo "==> [${region}] Restoring prod Lambdas..."

        local auth_integration users_integration
        auth_integration=$(get_integration_id "${region}" "${api_id}" "${hotfix_auth_arn}")
        users_integration=$(get_integration_id "${region}" "${api_id}" "${hotfix_users_arn}")

        if [[ -z "${auth_integration}" || -z "${users_integration}" ]]; then
            echo "Error: could not find hotfix integrations in ${region} — are they already restored?"
            exit 1
        fi

        update_integration "${region}" "${api_id}" "${auth_integration}" "${prod_auth_arn}"
        echo "    ✓ auth  → prod"

        update_integration "${region}" "${api_id}" "${users_integration}" "${prod_users_arn}"
        echo "    ✓ users → prod"
    fi
}

for r in "${REGIONS[@]}"; do
    swap_region "${r}"
    echo ""
done

if [[ "${ACTION}" == "activate" ]]; then
    echo "Hotfix active. Test at https://api.mendicant.io"
    echo "Restore when done: $0 deactivate ${REGION}"
else
    echo "Prod restored."
fi
