#!/bin/bash
# Setup Garage bucket with CORS for browser uploads

set -e

ADMIN_TOKEN="dev-admin-token"
GARAGE_ADMIN="http://localhost:3903"
BUCKET="poddyclip"

echo "Setting up Garage bucket: $BUCKET"

# Create bucket if not exists
echo "Creating bucket..."
curl -s -X POST "$GARAGE_ADMIN/v1/bucket" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"globalAliases\": [\"$BUCKET\"]}" || true

# Get bucket ID
BUCKET_ID=$(curl -s "$GARAGE_ADMIN/v1/bucket?globalAlias=$BUCKET" \
  -H "Authorization: Bearer $ADMIN_TOKEN" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

echo "Bucket ID: $BUCKET_ID"

# Grant key access to bucket
echo "Granting key access..."
curl -s -X POST "$GARAGE_ADMIN/v1/bucket/allow" \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"bucketId\": \"$BUCKET_ID\",
    \"accessKeyId\": \"GKc74972d763b00eb34ebb2cbf\",
    \"permissions\": {\"read\": true, \"write\": true, \"owner\": true}
  }"

echo ""
echo "Done! Bucket $BUCKET is ready with key access."
