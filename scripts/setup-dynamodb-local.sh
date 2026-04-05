#!/bin/bash
# Create DynamoDB tables for local development
# Run this after: docker compose up -d

set -e

ENDPOINT="http://localhost:8000"
REGION="us-east-2"
PROFILE="local.ddb"

echo "Creating DynamoDB Local tables..."

# Users table
aws dynamodb create-table \
  --table-name users \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
    AttributeName=sk,AttributeType=S \
    AttributeName=email,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
    AttributeName=sk,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v ResourceInUseException || true

# Add email-index GSI to users
aws dynamodb update-table \
  --table-name users \
  --attribute-definitions AttributeName=email,AttributeType=S \
  --global-secondary-index-updates "Create={IndexName=email-index,KeySchema=[{AttributeName=email,KeyType=HASH}],Projection={ProjectionType=ALL}}" \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v "ResourceInUseException\|ValidationException" || true

# Credentials table
aws dynamodb create-table \
  --table-name credentials \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
    AttributeName=sk,AttributeType=S \
    AttributeName=credential_id,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
    AttributeName=sk,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v ResourceInUseException || true

# Add credential_id-index GSI to credentials
aws dynamodb update-table \
  --table-name credentials \
  --attribute-definitions AttributeName=credential_id,AttributeType=S \
  --global-secondary-index-updates "Create={IndexName=credential-id-index,KeySchema=[{AttributeName=credential_id,KeyType=HASH}],Projection={ProjectionType=ALL}}" \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v "ResourceInUseException\|ValidationException" || true

# Refresh tokens table
aws dynamodb create-table \
  --table-name refresh_tokens \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
    AttributeName=user_id,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v ResourceInUseException || true

# Add user_id-index GSI to refresh_tokens
aws dynamodb update-table \
  --table-name refresh_tokens \
  --attribute-definitions AttributeName=user_id,AttributeType=S \
  --global-secondary-index-updates "Create={IndexName=user-index,KeySchema=[{AttributeName=user_id,KeyType=HASH}],Projection={ProjectionType=ALL}}" \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v "ResourceInUseException\|ValidationException" || true

# Challenges table (no GSI needed)
aws dynamodb create-table \
  --table-name challenges \
  --attribute-definitions AttributeName=pk,AttributeType=S \
  --key-schema AttributeName=pk,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v ResourceInUseException || true

# OAuth devices table
aws dynamodb create-table \
  --table-name oauth_devices \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
    AttributeName=user_code,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v ResourceInUseException || true

# Add user_code-index GSI to oauth_devices
aws dynamodb update-table \
  --table-name oauth_devices \
  --attribute-definitions AttributeName=user_code,AttributeType=S \
  --global-secondary-index-updates "Create={IndexName=user-code-index,KeySchema=[{AttributeName=user_code,KeyType=HASH}],Projection={ProjectionType=ALL}}" \
  --endpoint-url "$ENDPOINT" --region "$REGION" --profile "$PROFILE" 2>&1 | grep -v "ResourceInUseException\|ValidationException" || true

echo "✓ Tables created successfully!"
