# Rate Limiting Strategy

## Why It Matters

Poddyclip is a public audio processing service. Without rate limiting:

- **S3 costs** - Spam uploads fill storage, you pay
- **CPU exhaustion** - Flooded processing queue melts the Rust API
- **Service degradation** - Legitimate users can't process their audio

## Recommended Tool

**PlugAttack** with ETS storage.

- Built for Plug/Phoenix
- ETS is fast, in-memory, no external deps
- Simple rule-based configuration
- Good enough until multi-node deployment

## Recommended Limits

### Per-IP Limits (Anonymous)

| Endpoint | Limit | Period | Rationale |
|----------|-------|--------|-----------|
| `POST /api/presign` | 5 | 1 min | Uploads are expensive (S3 storage + triggers processing) |
| `POST /api/jobs` | 5 | 1 min | Each job is CPU-heavy on Rust API |
| `GET /api/*` | 60 | 1 min | General read endpoints, be generous |
| `DELETE /api/jobs/:id` | 10 | 1 min | Cancellations, shouldn't need many |
| WebSocket connect | 10 | 1 min | Prevent connection floods |

### Per-User Limits (Authenticated)

When auth is added, layer user-based limits on top of IP limits:

| User Type | Upload | Jobs | Rationale |
|-----------|--------|------|-----------|
| Free tier | 10/hour | 10/hour | Enough to try the service |
| Paid tier | 50/hour | 50/hour | Reasonable for regular use |
| Pro tier | 200/hour | 200/hour | Power users, batch processing |

## Implementation Approach

### 1. Add Dependency

```elixir
# mix.exs
{:plug_attack, "~> 0.4"}
```

### 2. Create Rate Limit Plug

```elixir
# lib/poddyclip_backend_web/plugs/rate_limit.ex
defmodule PoddyclipBackendWeb.Plugs.RateLimit do
  use PlugAttack
  import Plug.Conn

  # Strict limit for uploads (most expensive operation)
  rule "throttle uploads", conn do
    if conn.method == "POST" and conn.request_path == "/api/presign" do
      throttle(conn.remote_ip,
        period: 60_000,
        limit: 5,
        storage: {PlugAttack.Storage.Ets, __MODULE__.Storage}
      )
    end
  end

  # Strict limit for job creation
  rule "throttle job creation", conn do
    if conn.method == "POST" and conn.request_path == "/api/jobs" do
      throttle(conn.remote_ip,
        period: 60_000,
        limit: 5,
        storage: {PlugAttack.Storage.Ets, __MODULE__.Storage}
      )
    end
  end

  # General API throttle
  rule "throttle api", conn do
    if String.starts_with?(conn.request_path, "/api") do
      throttle(conn.remote_ip,
        period: 60_000,
        limit: 60,
        storage: {PlugAttack.Storage.Ets, __MODULE__.Storage}
      )
    end
  end

  # Allow everything else (static assets, etc.)
  rule "allow other", _conn do
    {:allow, nil}
  end

  # Custom block response
  def block_action(conn, _data, _opts) do
    conn
    |> put_resp_content_type("application/json")
    |> send_resp(429, Jason.encode!(%{error: "Too many requests. Please slow down."}))
    |> halt()
  end
end
```

### 3. Start ETS Table in Application

```elixir
# lib/poddyclip_backend/application.ex
children = [
  # ... existing children ...
  {PlugAttack.Storage.Ets, name: PoddyclipBackendWeb.Plugs.RateLimit.Storage, clean_period: 60_000}
]
```

### 4. Add to Router/Endpoint

```elixir
# lib/poddyclip_backend_web/endpoint.ex (or router.ex)
plug PoddyclipBackendWeb.Plugs.RateLimit
```

## Important Considerations

### IP Limits Have Holes

- Corporate NATs share IPs (100 users, 1 IP)
- VPNs and proxies mask real IPs
- Mobile carriers use CGNAT

**Mitigation:** When user is authenticated, use user ID as the throttle key instead of IP:

```elixir
rule "throttle authenticated uploads", conn do
  if conn.method == "POST" and conn.request_path == "/api/presign" do
    case get_session(conn, :user_id) do
      nil ->
        # Fall through to IP-based rule
        nil
      user_id ->
        throttle({:user, user_id},
          period: 3_600_000,  # 1 hour
          limit: 10,          # 10/hour for free tier
          storage: {PlugAttack.Storage.Ets, __MODULE__.Storage}
        )
    end
  end
end
```

### Logging Blocked Requests

Log when limits are hit to understand patterns:

```elixir
def block_action(conn, data, _opts) do
  require Logger
  Logger.warning("Rate limit hit", [
    ip: ip_to_string(conn.remote_ip),
    path: conn.request_path,
    rule: data[:rule],
    count: data[:count],
    limit: data[:limit]
  ])

  conn
  |> put_resp_content_type("application/json")
  |> send_resp(429, Jason.encode!(%{error: "Too many requests"}))
  |> halt()
end

defp ip_to_string(ip) when is_tuple(ip), do: :inet.ntoa(ip) |> to_string()
defp ip_to_string(ip), do: inspect(ip)
```

Review these logs periodically:
- Legitimate users hitting limits → limits too tight
- Same IPs repeatedly blocked → potential attack
- Bursts then silence → scripts/bots

### Start Strict, Loosen Later

It's easier to increase limits than to tighten them. Users complain less about "we increased your limits" than "we had to restrict your usage."

### Existing Protections

You already have some natural limits:

- **500MB file size cap** - Caps individual damage
- **Single job slot per session** - Can't queue unlimited jobs
- **Jobs tied to sessions** - Some accountability

Rate limiting adds defense in depth.

## Future Considerations

### Multi-Node Deployment

ETS is per-node. If you deploy 3 nodes behind a load balancer, a user could get 3x the limit by hitting different nodes.

Options:
1. **Sticky sessions** - Route same IP to same node (easiest)
2. **Redis storage** - Shared counter across nodes (PlugAttack supports custom storage)
3. **Accept it** - 3x limit might still be fine

### Graduated Responses

Instead of hard block at limit:

1. **Soft limit (80%)** - Add artificial delay (100ms)
2. **Hard limit (100%)** - Block with 429
3. **Abuse threshold (200%)** - Temporary IP ban (10 min)

### CAPTCHA Integration

For persistent abuse, require CAPTCHA verification before allowing more requests. Adds friction but stops most automated abuse.

## Summary

1. Add PlugAttack with ETS storage
2. Set strict initial limits (5/min for expensive operations)
3. Log all blocks to understand patterns
4. Layer user-based limits when auth exists
5. Revisit limits based on real usage data
