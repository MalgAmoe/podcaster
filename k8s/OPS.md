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
kubectl top pods  # requires metrics-server
```

## OpenObserve (Observability)

OpenObserve is deployed for internal logs/metrics. No external access - use SSH port-forward.

### Deploy (first time)

```bash
# 1. Create bucket 'openobserve-data' in Cloudflare R2

# 2. Create .env.openobserve with:
#    ZO_ROOT_USER_EMAIL=admin@yourdomain.com
#    ZO_ROOT_USER_PASSWORD=YourSecurePassword
#    ZO_S3_SERVER_URL=https://your-account.r2.cloudflarestorage.com
#    ZO_S3_ACCESS_KEY=your-r2-access-key
#    ZO_S3_SECRET_KEY=your-r2-secret-key

# 3. Create secret and deploy
kubectl create secret generic openobserve-secrets --from-env-file=.env.openobserve
kubectl apply -f k8s/openobserve.yaml

# 4. Verify
kubectl get pods -l app=openobserve
kubectl logs -l app=openobserve
```

### Access UI

```bash
# Option 1: From server
kubectl port-forward svc/openobserve 5080:5080
# Then open http://localhost:5080

# Option 2: SSH tunnel from local machine
ssh -L 5080:localhost:5080 user@server -t 'kubectl port-forward svc/openobserve 5080:5080'
# Then open http://localhost:5080 on your local machine
```

### Update Secrets

```bash
kubectl delete secret openobserve-secrets
kubectl create secret generic openobserve-secrets --from-env-file=.env.openobserve
kubectl rollout restart deploy/openobserve
```

### Ship App Logs to OpenObserve

Add to `app-secrets` (after OpenObserve is running):
```
OPENOBSERVE_URL=http://openobserve:5080
OPENOBSERVE_USER=admin@yourdomain.com
OPENOBSERVE_PASSWORD=YourSecurePassword
OPENOBSERVE_ORG=default
OPENOBSERVE_STREAM=poddyclip
```

Then restart phoenix:
```bash
kubectl rollout restart deploy/phoenix
```
