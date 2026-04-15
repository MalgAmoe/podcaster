# This file is responsible for configuring your application
# and its dependencies with the aid of the Config module.
#
# This configuration file is loaded before any dependency and
# is restricted to this project.

# General application configuration
import Config

config :poddyclip_backend, :scopes,
  user: [
    default: true,
    module: PoddyclipBackend.Accounts.Scope,
    assign_key: :current_scope,
    access_path: [:user, :id],
    schema_key: :user_id,
    schema_type: :id,
    schema_table: :users,
    test_data_fixture: PoddyclipBackend.AccountsFixtures,
    test_setup_helper: :register_and_log_in_user
  ]

config :poddyclip_backend,
  ecto_repos: [PoddyclipBackend.Repo],
  generators: [timestamp_type: :utc_datetime]

# Configure the endpoint
config :poddyclip_backend, PoddyclipBackendWeb.Endpoint,
  url: [host: "localhost"],
  adapter: Bandit.PhoenixAdapter,
  render_errors: [
    formats: [html: PoddyclipBackendWeb.ErrorHTML, json: PoddyclipBackendWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: PoddyclipBackend.PubSub,
  live_view: [signing_salt: "Z6+3ZWqJ"]

# Disable default esbuild - we use custom build script
config :esbuild, version: "0.25.4"

# Configure tailwind (the version is required)
config :tailwind,
  version: "4.1.12",
  poddyclip_backend: [
    args: ~w(
      --input=assets/css/app.css
      --output=priv/static/assets/css/app.css
    ),
    cd: Path.expand("..", __DIR__)
  ]

# Sentry error tracking (DSN loaded from env in runtime.exs)
config :sentry,
  enable_source_code_context: true,
  root_source_code_paths: [File.cwd!()],
  environment_name: config_env(),
  integrations: [
    oban: [
      capture_errors: true,
      cron: [enabled: true]
    ]
  ]

# Configure Elixir's Logger
config :logger,
  handlers: [{Sentry.LoggerHandler, []}]

config :logger, :default_formatter,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id, :job_id, :user_id]

# Filter sensitive parameters from Phoenix logs
config :phoenix, :filter_parameters, [
  "password",
  "secret",
  "token",
  "api_key",
  "webhook_secret",
  "download_url",
  "X-Amz-Signature",
  "X-Amz-Credential"
]

# Use Jason for JSON parsing in Phoenix
config :phoenix, :json_library, Jason

# Swoosh - disable API client for local adapter
config :swoosh, :api_client, false

# Oban job queue
config :poddyclip_backend, Oban,
  repo: PoddyclipBackend.Repo,
  queues: [default: 10, processing: 4],
  plugins: [
    Oban.Plugins.Pruner,
    {Oban.Plugins.Cron,
     crontab: [
       # Cleanup old jobs every hour
       {"0 * * * *", PoddyclipBackend.Workers.CleanupJobs},
       # Cleanup orphaned S3 files daily at 3am
       {"0 3 * * *", PoddyclipBackend.Workers.CleanupOrphanedFiles},
       # Cleanup expired minute packs daily at 4am
       {"0 4 * * *", PoddyclipBackend.Workers.CleanupExpiredPacks}
     ]}
  ]

# Cleanup configuration
config :poddyclip_backend, :cleanup,
  job_retention_days: 7,
  stale_job_hours: 2

# Admin routes - disabled by default (compile-time setting)
# Set ADMIN_ENABLED=true at BUILD time to include admin routes in the release
config :poddyclip_backend, admin_enabled: System.get_env("ADMIN_ENABLED") == "true"

# Admin endpoint (separate from main, localhost only for SSH tunnel access)
config :poddyclip_backend, PoddyclipBackendWeb.AdminEndpoint,
  url: [host: "localhost"],
  adapter: Bandit.PhoenixAdapter,
  render_errors: [
    formats: [html: PoddyclipBackendWeb.ErrorHTML, json: PoddyclipBackendWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: PoddyclipBackend.PubSub,
  live_view: [signing_salt: "AdminLV01"]

# Import environment specific config. This must remain at the bottom
# of this file so it overrides the configuration defined above.
import_config "#{config_env()}.exs"
