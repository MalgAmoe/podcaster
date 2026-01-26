defmodule PoddyclipBackend.Application do
  # See https://hexdocs.pm/elixir/Application.html
  # for more information on OTP Applications
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      PoddyclipBackendWeb.Telemetry,
      PoddyclipBackend.Repo,
      {DNSCluster, query: Application.get_env(:poddyclip_backend, :dns_cluster_query) || :ignore},
      {Phoenix.PubSub, name: PoddyclipBackend.PubSub},
      # Log shipper for OpenObserve (must start before logger backend uses it)
      PoddyclipBackend.LogShipper,
      # Oban job queue
      {Oban, Application.fetch_env!(:poddyclip_backend, Oban)},
      # Start to serve requests, typically the last entry
      PoddyclipBackendWeb.Endpoint
    ]

    # See https://hexdocs.pm/elixir/Supervisor.html
    # for other strategies and supported options
    opts = [strategy: :one_for_one, name: PoddyclipBackend.Supervisor]
    Supervisor.start_link(children, opts)
  end

  # Tell Phoenix to update the endpoint configuration
  # whenever the application is updated.
  @impl true
  def config_change(changed, _new, removed) do
    PoddyclipBackendWeb.Endpoint.config_change(changed, removed)
    :ok
  end
end
