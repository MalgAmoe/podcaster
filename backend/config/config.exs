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

# Configure esbuild (the version is required)
config :esbuild,
  version: "0.25.4",
  poddyclip_backend: [
    args:
      ~w(js/app.js --bundle --target=es2022 --outdir=../priv/static/assets/js --external:/fonts/* --external:/images/* --alias:@=.),
    cd: Path.expand("../assets", __DIR__),
    env: %{"NODE_PATH" => [Path.expand("../deps", __DIR__), Mix.Project.build_path()]}
  ]

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

# Configure Elixir's Logger
config :logger, :default_formatter,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id]

# Use Jason for JSON parsing in Phoenix
config :phoenix, :json_library, Jason

# Swoosh - disable API client for local adapter
config :swoosh, :api_client, false

# Oban job queue
config :poddyclip_backend, Oban,
  repo: PoddyclipBackend.Repo,
  queues: [processing: 4]

# Import environment specific config. This must remain at the bottom
# of this file so it overrides the configuration defined above.
import_config "#{config_env()}.exs"
