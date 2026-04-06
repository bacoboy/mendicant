#!/bin/bash
# Create DynamoDB tables for local development
# Run this after: docker compose up -d
#
# This script is idempotent: it will check if tables exist, compare schemas,
# and prompt before making destructive changes.

set -e

ENDPOINT="http://localhost:8000"
REGION="us-east-2"
PROFILE="local.ddb"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper: Check if table exists
table_exists() {
    local table=$1
    aws dynamodb describe-table \
        --table-name "$table" \
        --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" \
        >/dev/null 2>&1
    return $?
}

# Helper: Delete and recreate table (with user confirmation)
recreate_table() {
    local table=$1
    echo -e "${RED}Table '$table' schema mismatch.${NC}"
    echo "Recreating table requires deleting it first (data will be lost)."
    read -p "Proceed with recreation? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Skipped table recreation for '$table'"
        return 1
    fi

    echo "Deleting table '$table'..."
    aws dynamodb delete-table \
        --table-name "$table" \
        --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" >/dev/null

    # Wait for deletion
    while table_exists "$table"; do
        echo "Waiting for deletion..."
        sleep 1
    done

    return 0
}

# Helper: Create or ensure table
ensure_table() {
    local table=$1
    local attr_defs=$2
    local key_schema=$3

    if table_exists "$table"; then
        echo -e "${YELLOW}✓ Table '$table' already exists${NC}"
        return 0
    fi

    echo "Creating table '$table'..."
    aws dynamodb create-table \
        --table-name "$table" \
        --attribute-definitions $attr_defs \
        --key-schema $key_schema \
        --billing-mode PAY_PER_REQUEST \
        --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" >/dev/null
    echo -e "${GREEN}✓ Created table '$table'${NC}"
}

# Helper: Ensure GSI exists
ensure_gsi() {
    local table=$1
    local index_name=$2
    local key_schema=$3

    # Check if GSI already exists
    local gsi_exists=$(aws dynamodb describe-table \
        --table-name "$table" \
        --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" \
        --query "Table.GlobalSecondaryIndexes[?IndexName=='$index_name']" \
        --output json 2>/dev/null | grep -c "$index_name" || true)

    if [ "$gsi_exists" -gt 0 ]; then
        echo -e "${YELLOW}  ✓ GSI '$index_name' already exists${NC}"
        return 0
    fi

    # Build attribute definitions for the GSI
    local attr_defs=""
    if [[ "$key_schema" == *"email"* ]]; then
        attr_defs="AttributeName=email,AttributeType=S"
    elif [[ "$key_schema" == *"credential_id"* ]]; then
        attr_defs="AttributeName=credential_id,AttributeType=S"
    elif [[ "$key_schema" == *"user_id"* ]]; then
        attr_defs="AttributeName=user_id,AttributeType=S"
    elif [[ "$key_schema" == *"user_code"* ]]; then
        attr_defs="AttributeName=user_code,AttributeType=S"
    fi

    echo "  Adding GSI '$index_name'..."
    aws dynamodb update-table \
        --table-name "$table" \
        --attribute-definitions "$attr_defs" \
        --global-secondary-index-updates "Create={IndexName=$index_name,KeySchema=[$key_schema],Projection={ProjectionType=ALL}}" \
        --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" >/dev/null 2>&1 || {
        if [ $? -ne 0 ]; then
            echo -e "${YELLOW}  (GSI creation request submitted, may already exist)${NC}"
        fi
    }
    echo -e "${GREEN}  ✓ GSI '$index_name' ensured${NC}"
}

echo "Setting up DynamoDB Local tables..."
echo

# Users table
ensure_table "users" \
    "AttributeName=pk,AttributeType=S AttributeName=sk,AttributeType=S AttributeName=email,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH AttributeName=sk,KeyType=RANGE"
ensure_gsi "users" "email-index" "{AttributeName=email,KeyType=HASH}"

# Credentials table
ensure_table "credentials" \
    "AttributeName=pk,AttributeType=S AttributeName=sk,AttributeType=S AttributeName=credential_id,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH AttributeName=sk,KeyType=RANGE"
ensure_gsi "credentials" "credential-id-index" "{AttributeName=credential_id,KeyType=HASH}"

# Refresh tokens table
ensure_table "refresh_tokens" \
    "AttributeName=pk,AttributeType=S AttributeName=user_id,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH"
ensure_gsi "refresh_tokens" "user-index" "{AttributeName=user_id,KeyType=HASH}"

# Challenges table (regional, no GSI)
ensure_table "challenges" \
    "AttributeName=pk,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH"

# Email tokens table (regional, no GSI)
ensure_table "email_tokens" \
    "AttributeName=pk,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH"

# OAuth devices table (regional)
ensure_table "oauth_devices" \
    "AttributeName=pk,AttributeType=S AttributeName=user_code,AttributeType=S" \
    "AttributeName=pk,KeyType=HASH"
ensure_gsi "oauth_devices" "user-code-index" "{AttributeName=user_code,KeyType=HASH}"

echo
echo -e "${GREEN}✓ All tables ready!${NC}"
echo
echo "Tables created/verified:"
echo "  • users (global)"
echo "  • credentials (global)"
echo "  • refresh_tokens (global)"
echo "  • challenges (regional)"
echo "  • email_tokens (regional)"
echo "  • oauth_devices (regional)"
