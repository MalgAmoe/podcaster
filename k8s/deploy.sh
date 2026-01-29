#!/bin/bash
# Rolling deploy - triggers k8s to pull new images and do zero-downtime update
set -e

echo "=== Deploying to k8s ==="

# Pull latest images (so k8s can see the new digest)
echo "Pulling latest images..."
docker pull malgamoe/stuff:api
docker pull malgamoe/stuff:phoenix

# Restart deployments (triggers rolling update with new image)
echo "Rolling out api..."
kubectl rollout restart deployment/api

echo "Rolling out phoenix..."
kubectl rollout restart deployment/phoenix

# Watch the rollout
echo ""
echo "Watching rollout status..."
kubectl rollout status deployment/api
kubectl rollout status deployment/phoenix

echo ""
echo "=== Deploy complete! ==="
kubectl get pods
