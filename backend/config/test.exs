import Config

# Only in tests, remove the complexity from the password hashing algorithm
config :bcrypt_elixir, :log_rounds, 1

# We don't run a server during test. If one is required,
# you can enable the server option below.
config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: "hj6fM3jOlTv4yQ24/50L/WwEWLXuUAi3z13ebYQREu5GfgLPw/fKevVjhZjeWnBa",
  server: false

# Database configuration for tests
config :poddyclip_backend, PoddyclipBackend.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "poddyclip_backend_test#{System.get_env("MIX_TEST_PARTITION")}",
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: System.schedulers_online() * 2

# Oban - run jobs inline in tests
config :poddyclip_backend, Oban, testing: :inline

# Use test adapter for Mailer
config :poddyclip_backend, PoddyclipBackend.Mailer, adapter: Swoosh.Adapters.Test

# Print only warnings and errors during test
config :logger, level: :warning

# Initialize plugs at runtime for faster test compilation
config :phoenix, :plug_init_mode, :runtime

# Enable helpful, but potentially expensive runtime checks
config :phoenix_live_view,
  enable_expensive_runtime_checks: true

# Sort query params output of verified routes for robust url comparisons
config :phoenix,
  sort_verified_routes_query_params: true
