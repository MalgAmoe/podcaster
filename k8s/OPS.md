# K8s Operations Guide

## Quick Reference

| Task | Command |
|------|---------|
| Deploy new code | `./k8s/deploy.sh` |
| Check status | `kubectl get pods` |
| View logs | `kubectl logs -f deploy/phoenix` |
| Restart app | `kubectl rollout restart deploy/phoenix` |

## Initial Setup (first time only)

```bash
# From server, with .env.prod file ready
./k8s/setup.sh .env.prod
```

This creates secrets and applies all manifests.

## Deploy Updates

After pushing new Docker images:

```bash
./k8s/deploy.sh
```

This pulls latest images and does rolling restart.

## Logs

```bash
# Phoenix logs (follow)
kubectl logs -f deploy/phoenix

# API logs (follow)
kubectl logs -f deploy/api

# Last 100 lines
kubectl logs deploy/phoenix --tail=100

# Previous crashed pod
kubectl logs deploy/phoenix --previous
```

## Status & Debugging

```bash
# Pod status
kubectl get pods

# Detailed pod info (see why it's failing)
kubectl describe pod <pod-name>

# Get into a running container
kubectl exec -it deploy/phoenix -- sh
kubectl exec -it deploy/api -- sh

# Check services
kubectl get svc

# Check ingress
kubectl get ingress
```

## Restart / Rollback

```bash
# Restart (re-pulls image, zero downtime)
kubectl rollout restart deploy/phoenix
kubectl rollout restart deploy/api

# Watch rollout progress
kubectl rollout status deploy/phoenix

# Rollback to previous version
kubectl rollout undo deploy/phoenix
```

## Update Secrets

```bash
# Edit .env.prod, then:
kubectl delete secret app-secrets
kubectl create secret generic app-secrets --from-env-file=.env.prod

# Restart to pick up new secrets
kubectl rollout restart deploy/phoenix
kubectl rollout restart deploy/api
```

## Destroy Everything

```bash
# Delete deployments and services
kubectl delete -f k8s/api.yaml
kubectl delete -f k8s/phoenix.yaml

# Delete secrets
kubectl delete secret app-secrets

# Nuclear option: delete namespace (if using one)
# kubectl delete namespace <ns>
```

## Traefik (ingress controller)

```bash
# Check traefik pods
kubectl get pods -n kube-system | grep traefik

# Restart traefik
kubectl rollout restart deploy/traefik -n kube-system

# Traefik config is at:
# /var/lib/rancher/k3s/server/manifests/traefik-config.yaml
```

## Troubleshooting

**Pod stuck in Pending:**
```bash
kubectl describe pod <name>  # check Events section
```

**Pod in CrashLoopBackOff:**
```bash
kubectl logs <pod-name> --previous  # see crash logs
```

**Can't connect to service:**
```bash
# Test from inside cluster
kubectl run -it --rm debug --image=curlimages/curl -- sh
curl http://phoenix:4000/health
```

**Check resource usage:**
```bash
kubectl top pods   # requires metrics-server
```

## Server Shutdown & Restart
```bash
kubectl rollout restart deploy/phoenix
```

## Server Shutdown & Restart

This section covers safely shutting down and restarting the entire k3s server.

### Architecture Overview

**Stateless design**: All persistent state is external:
- **Database**: PostgreSQL (external)
- **Storage**: Cloudflare R2

**Service Dependencies**:
```
Phoenix (port 4000) → API (port 3000) → R2 Storage
```

**Graceful Shutdown Periods**:
- Phoenix: 30s
- API: 600s (10 min) - has preStop hook that waits for active jobs

### Graceful Shutdown Procedure

#### 1. Check for Active Jobs

```bash
# Check API health (shows active job count)
kubectl exec deploy/api -- curl -s localhost:3000/health

# Check API logs for job activity
kubectl logs deploy/api --tail=50 | grep -i "processing\|job"
```

If jobs are active, either:
- Wait for them to complete naturally
- Scale down to stop accepting new work: `kubectl scale deploy/api --replicas=0`

#### 2. Delete Deployments (in order)

```bash
# Stop accepting new requests first
kubectl delete deploy/phoenix

# Wait for API to drain (has 10-min grace period for active jobs)
kubectl delete deploy/api

# Verify all pods terminated
kubectl get pods
```

#### 3. (Optional) Delete Secrets

Only if server going offline long-term or decommissioning:
```bash
kubectl delete secret app-secrets
```

### Startup Procedure

#### 1. Verify External Dependencies

```bash
# Test Postgres connectivity (from a machine with psql)
psql $DATABASE_URL -c "SELECT 1"

# Test R2 (check bucket exists in Cloudflare dashboard)
```

#### 2. Recreate Secrets (if deleted)

```bash
# From directory with .env files
kubectl create secret generic app-secrets --from-env-file=.env.prod
```

#### 3. Apply Manifests (in order)

```bash
# Start API (backend)
kubectl apply -f k8s/api.yaml
kubectl rollout status deploy/api

# Start Phoenix (frontend) last
kubectl apply -f k8s/phoenix.yaml
kubectl rollout status deploy/phoenix
```

#### 4. Verify Health

```bash
# Check all pods running
kubectl get pods

# Test health endpoints
kubectl exec deploy/api -- curl -s localhost:3000/health
kubectl exec deploy/phoenix -- curl -s localhost:4000/health

# Check logs for errors
kubectl logs deploy/phoenix --tail=20
kubectl logs deploy/api --tail=20
```

### Quick Commands Reference

**Safe Shutdown (one-liner)**:
```bash
kubectl delete deploy/phoenix && \
kubectl delete deploy/api && \
kubectl get pods
```

**Full Startup (one-liner)**:
```bash
kubectl apply -f k8s/api.yaml && \
kubectl rollout status deploy/api && \
kubectl apply -f k8s/phoenix.yaml && \
kubectl rollout status deploy/phoenix
```

**Quick Health Check**:
```bash
kubectl get pods && \
kubectl exec deploy/api -- curl -s localhost:3000/health && \
kubectl exec deploy/phoenix -- curl -s localhost:4000/health
```

### Emergency Procedures

**Force kill stuck pods** (if graceful shutdown hangs):
```bash
kubectl delete pod <pod-name> --grace-period=0 --force
```

**Full reset** (nuclear option):
```bash
# Delete everything
kubectl delete deploy --all
kubectl delete svc --all
kubectl delete ingress --all
kubectl delete secret app-secrets

# Recreate from scratch
./k8s/setup.sh .env.prod
```

**Check k3s service**:
```bash
# Status
sudo systemctl status k3s

# Restart k3s itself (if needed)
sudo systemctl restart k3s

# Logs
sudo journalctl -u k3s -f
```
