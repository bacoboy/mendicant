#!/bin/bash
# Delete all test-* tables from DynamoDB Local

ENDPOINT="http://localhost:8000"
PROFILE="local.ddb"

echo "Fetching test tables..."
tables=$(aws dynamodb list-tables \
  --endpoint-url "$ENDPOINT" \
  --profile "$PROFILE" \
  --query "TableNames[?starts_with(@, 'test-')]" \
  --output text)

count=$(echo "$tables" | wc -w)
echo "Found $count test tables to delete"

if [ "$count" -eq 0 ]; then
  echo "No test tables to clean up"
  exit 0
fi

echo "Deleting test tables..."
for table in $tables; do
  echo "  Deleting $table..."
  aws dynamodb delete-table \
    --table-name "$table" \
    --endpoint-url "$ENDPOINT" \
    --profile "$PROFILE" \
    2>/dev/null || true
done

echo "✓ Cleanup complete"
