import Config

# Load .env file from project root (shared with Rust API)
# __DIR__ = backend/config, so go up twice to get project root
project_root = __DIR__ |> Path.dirname() |> Path.dirname()
env_file = Path.join(project_root, ".env")
env_local = Path.join(project_root, ".env.local")

# Dotenvy.source! returns a map but doesn't set env vars, so we do it manually
env_vars =
  cond do
    File.exists?(env_local) -> Dotenvy.source!(env_local)
    File.exists?(env_file) -> Dotenvy.source!(env_file)
    true -> %{}
  end

Enum.each(env_vars, fn {k, v} -> System.put_env(k, v) end)

# config/runtime.exs is executed for all environments, including
# during releases. It is executed after compilation and before the
# system starts, so it is typically used to load production configuration
# and secrets from environment variables or elsewhere. Do not define
# any compile-time configuration in here, as it won't be applied.
# The block below contains prod specific runtime configuration.

# ## Using releases
#
# If you use `mix release`, you need to explicitly enable the server
# by passing the PHX_SERVER=true when you start it:
#
#     PHX_SERVER=true bin/poddyclip_backend start
#
# Alternatively, you can use `mix phx.gen.release` to generate a `bin/server`
# script that automatically sets the env var above.
if System.get_env("PHX_SERVER") do
  config :poddyclip_backend, PoddyclipBackendWeb.Endpoint, server: true
end

config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
  http: [port: String.to_integer(System.get_env("PORT", "4000"))]

# Allow overriding the URL host for email links (useful with tunnels like Cloudflare)
if phx_host = System.get_env("PHX_HOST") do
  config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
    url: [host: phx_host, scheme: "https", port: 443]
end

# Basic auth for pre-launch testing (set SITE_PASSWORD to enable)
if site_password = System.get_env("SITE_PASSWORD") do
  config :poddyclip_backend,
    site_password: site_password,
    site_username: System.get_env("SITE_USERNAME", "admin")
end

# S3 Configuration (MinIO or AWS S3 compatible)
# Supports S3_ENDPOINT (full URL) or S3_HOST (just hostname)
s3_endpoint = System.get_env("S3_ENDPOINT") || System.get_env("S3_HOST")

if s3_endpoint do
  # Parse endpoint URL to extract scheme, host, port
  uri = URI.parse(if String.starts_with?(s3_endpoint, "http"), do: s3_endpoint, else: "http://#{s3_endpoint}")
  s3_scheme = "#{uri.scheme}://"
  s3_host = uri.host || s3_endpoint
  s3_port = uri.port || (if uri.scheme == "https", do: 443, else: 80)

  config :ex_aws,
    access_key_id: System.get_env("S3_ACCESS_KEY"),
    secret_access_key: System.get_env("S3_SECRET_KEY"),
    region: System.get_env("S3_REGION", "us-east-1")

  config :ex_aws, :s3,
    scheme: s3_scheme,
    host: s3_host,
    port: s3_port

  config :poddyclip_backend, :s3,
    bucket: System.get_env("S3_BUCKET", "poddyclip"),
    enabled: true
else
  config :poddyclip_backend, :s3, enabled: false
end

# Poddyclip API service URL (Rust processing service)
if api_url = System.get_env("PODDYCLIP_API_URL") do
  config :poddyclip_backend, :poddyclip_api_url, api_url
end

# Webhook base URL (how Rust API calls back to Phoenix)
if webhook_url = System.get_env("WEBHOOK_BASE_URL") do
  config :poddyclip_backend, :webhook_base_url, webhook_url
end

# API key for authenticating with the Rust poddyclip-api service
if api_key = System.get_env("API_KEY") do
  config :poddyclip_backend, :api_key, api_key
end

# Webhook secret for authenticating incoming webhooks from Rust API
if webhook_secret = System.get_env("WEBHOOK_SECRET") do
  config :poddyclip_backend, :webhook_secret, webhook_secret
end

# Polar billing configuration
if polar_webhook_secret = System.get_env("POLAR_WEBHOOK_SECRET") do
  config :poddyclip_backend, :polar_webhook_secret, polar_webhook_secret
end

# Polar organization slug (default: munchy-cow)
if polar_org = System.get_env("POLAR_ORGANIZATION") do
  config :poddyclip_backend, :polar_organization, polar_org
end

# Polar host (default: polar.sh, use sandbox.polar.sh for testing)
if polar_host = System.get_env("POLAR_HOST") do
  config :poddyclip_backend, :polar_host, polar_host
end

# Polar checkout link ID (from Polar dashboard checkout links)
if polar_checkout_link_id = System.get_env("POLAR_CHECKOUT_LINK_ID") do
  config :poddyclip_backend, :polar_checkout_link_id, polar_checkout_link_id
end

# Polar API access token (for fetching subscription status)
if polar_access_token = System.get_env("POLAR_ACCESS_TOKEN") do
  config :poddyclip_backend, :polar_access_token, polar_access_token
end

# Polar minute pack product ID (for verifying order webhooks)
if polar_minute_pack_product_id = System.get_env("POLAR_MINUTE_PACK_PRODUCT_ID") do
  config :poddyclip_backend, :polar_minute_pack_product_id, polar_minute_pack_product_id
end

# Polar minute pack checkout link ID (for purchase URLs)
if polar_minute_pack_checkout_link_id = System.get_env("POLAR_MINUTE_PACK_CHECKOUT_LINK_ID") do
  config :poddyclip_backend, :polar_minute_pack_checkout_link_id, polar_minute_pack_checkout_link_id
end


# OpenObserve log shipping (if configured)
if openobserve_url = System.get_env("OPENOBSERVE_URL") do
  config :poddyclip_backend, :openobserve,
    url: openobserve_url,
    user: System.get_env("OPENOBSERVE_USER", "admin@poddyclip.local"),
    password: System.get_env("OPENOBSERVE_PASSWORD", "dev"),
    org: System.get_env("OPENOBSERVE_ORG", "default"),
    stream: System.get_env("OPENOBSERVE_STREAM", "phoenix")

  # Add OpenObserve logger backend
  config :logger,
    backends: [:console, PoddyclipBackend.LogShipper.Backend]
end

# Admin auth credentials and endpoint (only used if admin routes were compiled in)
# ADMIN_ENABLED is a compile-time setting, but we still check it here
# to avoid errors when admin routes aren't available
if Application.compile_env(:poddyclip_backend, :admin_enabled) do
  admin_username = System.get_env("ADMIN_USERNAME", "admin")
  admin_password = System.get_env("ADMIN_PASSWORD")
  admin_port = String.to_integer(System.get_env("ADMIN_PORT", "4001"))

  if config_env() == :prod and is_nil(admin_password) do
    raise """
    environment variable ADMIN_PASSWORD is missing.
    This is required for admin routes in production.
    """
  end

  config :poddyclip_backend,
    admin_username: admin_username,
    admin_password: admin_password || "dev"

  # Admin endpoint on separate port, bound to localhost only (access via SSH tunnel)
  # In dev, also enable server. In prod, controlled by PHX_SERVER.
  config :poddyclip_backend, PoddyclipBackendWeb.AdminEndpoint,
    http: [
      ip: {127, 0, 0, 1},
      port: admin_port
    ],
    server: config_env() == :dev or System.get_env("PHX_SERVER") != nil,
    secret_key_base: System.get_env("SECRET_KEY_BASE") || "dev-secret-key-base-for-admin-at-least-64-bytes-long-for-security"
end

if config_env() == :prod do
  # Sentry error tracking
  if sentry_dsn = System.get_env("SENTRY_DSN") do
    config :sentry, dsn: sentry_dsn
  end

  # Email via Resend
  resend_api_key = System.get_env("RESEND_API_KEY")

  if resend_api_key do
    config :poddyclip_backend, PoddyclipBackend.Mailer,
      adapter: Swoosh.Adapters.Resend,
      api_key: resend_api_key

    # Use Hackney as the HTTP client for Swoosh in production
    config :swoosh, :api_client, Swoosh.ApiClient.Hackney

    # Configure from email (must be verified in Resend)
    config :poddyclip_backend, :email_from,
      name: System.get_env("EMAIL_FROM_NAME", "Munchy Cow"),
      address: System.get_env("EMAIL_FROM_ADDRESS", "noreply@munchycow.com")
  end

  database_url =
    System.get_env("DATABASE_URL") ||
      raise """
      environment variable DATABASE_URL is missing.
      For example: ecto://USER:PASS@HOST/DATABASE
      """

  config :poddyclip_backend, PoddyclipBackend.Repo,
    url: database_url,
    pool_size: String.to_integer(System.get_env("POOL_SIZE") || "10"),
    ssl: [verify: :verify_none]

  # The secret key base is used to sign/encrypt cookies and other secrets.
  # A default value is used in config/dev.exs and config/test.exs but you
  # want to use a different value for prod and you most likely don't want
  # to check this value into version control, so we use an environment
  # variable instead.
  secret_key_base =
    System.get_env("SECRET_KEY_BASE") ||
      raise """
      environment variable SECRET_KEY_BASE is missing.
      You can generate one by calling: mix phx.gen.secret
      """

  host = System.get_env("PHX_HOST") || "localhost:4000"

  config :poddyclip_backend, :dns_cluster_query, System.get_env("DNS_CLUSTER_QUERY")

  config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
    url: [host: host, port: 443, scheme: "https"],
    check_origin: [
      "https://#{host}",
      "https://www.#{host}",
      "https://munchycow.com",
      "https://www.munchycow.com"
    ],
    http: [
      # Enable IPv6 and bind on all interfaces.
      # Set it to  {0, 0, 0, 0, 0, 0, 0, 1} for local network only access.
      # See the documentation on https://hexdocs.pm/bandit/Bandit.html#t:options/0
      # for details about using IPv6 vs IPv4 and loopback vs public addresses.
      ip: {0, 0, 0, 0, 0, 0, 0, 0}
    ],
    secret_key_base: secret_key_base

  # ## SSL Support
  #
  # To get SSL working, you will need to add the `https` key
  # to your endpoint configuration:
  #
  #     config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
  #       https: [
  #         ...,
  #         port: 443,
  #         cipher_suite: :strong,
  #         keyfile: System.get_env("SOME_APP_SSL_KEY_PATH"),
  #         certfile: System.get_env("SOME_APP_SSL_CERT_PATH")
  #       ]
  #
  # The `cipher_suite` is set to `:strong` to support only the
  # latest and more secure SSL ciphers. This means old browsers
  # and clients may not be supported. You can set it to
  # `:compatible` for wider support.
  #
  # `:keyfile` and `:certfile` expect an absolute path to the key
  # and cert in disk or a relative path inside priv, for example
  # "priv/ssl/server.key". For all supported SSL configuration
  # options, see https://hexdocs.pm/plug/Plug.SSL.html#configure/1
  #
  # We also recommend setting `force_ssl` in your config/prod.exs,
  # ensuring no data is ever sent via http, always redirecting to https:
  #
  #     config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
  #       force_ssl: [hsts: true]
  #
  # Check `Plug.SSL` for all available options in `force_ssl`.
end
