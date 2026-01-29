#!/bin/bash
# Setup k8s secrets from .env.prod
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="${1:-.env.prod}"

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: $ENV_FILE not found"
    echo "Usage: ./setup.sh [path-to-env-file]"
    exit 1
fi

echo "Creating secret from $ENV_FILE..."

# Delete existing secret if exists
kubectl delete secret app-secrets 2>/dev/null || true

# Create secret from env file
kubectl create secret generic app-secrets --from-env-file="$ENV_FILE"

echo "Applying manifests..."
kubectl apply -f "$SCRIPT_DIR/api.yaml"
kubectl apply -f "$SCRIPT_DIR/phoenix.yaml"

echo ""
echo "Done! Check status with:"
echo "  kubectl get pods"
echo "  kubectl get services"
