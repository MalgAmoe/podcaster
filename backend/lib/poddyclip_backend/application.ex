defmodule PoddyclipBackend.Application do
  # See https://hexdocs.pm/elixir/Application.html
  # for more information on OTP Applications
  @moduledoc false

  use Application

  @admin_enabled Application.compile_env(:poddyclip_backend, :admin_enabled, false)

  @impl true
  def start(_type, _args) do
    children =
      [
        PoddyclipBackendWeb.Telemetry,
        PoddyclipBackend.Repo,
        {DNSCluster, query: Application.get_env(:poddyclip_backend, :dns_cluster_query) || :ignore},
        {Phoenix.PubSub, name: PoddyclipBackend.PubSub},
        # Log shipper for OpenObserve (must start before logger backend uses it)
        PoddyclipBackend.LogShipper,
        # Rate limiter for data exports
        PoddyclipBackend.RateLimiter,
        # Oban job queue
        {Oban, Application.fetch_env!(:poddyclip_backend, Oban)},
        # Subscription expiry checker
        PoddyclipBackend.Workers.SubscriptionExpiryWorker,
        # Expiry notification sender (7 days before subscription ends)
        PoddyclipBackend.Workers.ExpiryNotificationWorker,
        # Free plan monthly reset checker
        PoddyclipBackend.Workers.FreePlanResetWorker,
        # Start to serve requests, typically the last entry
        PoddyclipBackendWeb.Endpoint
      ] ++ admin_children()

    # See https://hexdocs.pm/elixir/Supervisor.html
    # for other strategies and supported options
    opts = [strategy: :one_for_one, name: PoddyclipBackend.Supervisor]
    Supervisor.start_link(children, opts)
  end

  # Admin endpoint on separate port (localhost only, access via SSH tunnel)
  if @admin_enabled do
    defp admin_children do
      [PoddyclipBackendWeb.AdminEndpoint]
    end
  else
    defp admin_children, do: []
  end

  # Tell Phoenix to update the endpoint configuration
  # whenever the application is updated.
  @impl true
  def config_change(changed, _new, removed) do
    PoddyclipBackendWeb.Endpoint.config_change(changed, removed)

    if @admin_enabled do
      PoddyclipBackendWeb.AdminEndpoint.config_change(changed, removed)
    end

    :ok
  end
end
