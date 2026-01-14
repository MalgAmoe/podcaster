#!/bin/bash
set -e

ADMIN_TOKEN="dev-admin-token"
GARAGE_ADMIN="http://localhost:3903"

echo "Waiting for Garage to start..."
until curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$GARAGE_ADMIN/v1/health" > /dev/null 2>&1; do
    sleep 1
done
echo "Garage is up!"

# Get node ID
NODE_ID=$(curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$GARAGE_ADMIN/v1/status" | jq -r '.node')
echo "Node ID: $NODE_ID"

# Configure the node with capacity (1GB for dev)
echo "Configuring node layout..."
curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    "$GARAGE_ADMIN/v1/layout" \
    -d "{\"version\": null, \"roles\": {\"$NODE_ID\": {\"zone\": \"dc1\", \"capacity\": 1073741824, \"tags\": []}}}" > /dev/null

# Apply layout
CURRENT_VERSION=$(curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$GARAGE_ADMIN/v1/layout" | jq -r '.version')
NEXT_VERSION=$((CURRENT_VERSION + 1))

curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    "$GARAGE_ADMIN/v1/layout/apply" \
    -d "{\"version\": $NEXT_VERSION}" > /dev/null

echo "Layout applied!"

# Create bucket
echo "Creating bucket 'poddyclip'..."
curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    "$GARAGE_ADMIN/v1/bucket" \
    -d '{"globalAliases": ["poddyclip"]}' > /dev/null 2>&1 || true

# Get bucket ID
BUCKET_ID=$(curl -s -H "Authorization: Bearer $ADMIN_TOKEN" "$GARAGE_ADMIN/v1/bucket?globalAlias=poddyclip" | jq -r '.id')
echo "Bucket ID: $BUCKET_ID"

# Create API key
echo "Creating API key..."
KEY_RESPONSE=$(curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    "$GARAGE_ADMIN/v1/key" \
    -d '{"name": "poddyclip-dev"}')

ACCESS_KEY=$(echo "$KEY_RESPONSE" | jq -r '.accessKeyId')
SECRET_KEY=$(echo "$KEY_RESPONSE" | jq -r '.secretAccessKey')

# Grant permissions to key
KEY_ID=$(echo "$KEY_RESPONSE" | jq -r '.id')
curl -s -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    "$GARAGE_ADMIN/v1/bucket/allow" \
    -d "{\"bucketId\": \"$BUCKET_ID\", \"accessKeyId\": \"$KEY_ID\", \"permissions\": {\"read\": true, \"write\": true, \"owner\": true}}" > /dev/null

echo ""
echo "=== Garage Setup Complete ==="
echo ""
echo "Add these to your environment or .env file:"
echo ""
echo "S3_ENDPOINT=http://localhost:3900"
echo "S3_BUCKET=poddyclip"
echo "S3_REGION=garage"
echo "S3_ACCESS_KEY=$ACCESS_KEY"
echo "S3_SECRET_KEY=$SECRET_KEY"
echo ""
