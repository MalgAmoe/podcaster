defmodule PoddyclipBackendWeb.AdminRouter do
  @moduledoc """
  Router for admin-only routes, served on the internal AdminEndpoint.
  """

  use PoddyclipBackendWeb, :router
  import Phoenix.LiveDashboard.Router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PoddyclipBackendWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  pipeline :admin_auth do
    plug PoddyclipBackendWeb.Plugs.AdminAuth
  end

  # Admin API endpoints (JSON)
  scope "/admin/api", PoddyclipBackendWeb do
    pipe_through [:api, :admin_auth]

    get "/health", AdminApiController, :health
    get "/jobs/stats", AdminApiController, :job_stats
    get "/jobs/active", AdminApiController, :active_jobs
    get "/errors", AdminApiController, :errors
    get "/users/stats", AdminApiController, :user_stats
  end

  # Admin LiveDashboard
  scope "/admin" do
    pipe_through [:browser, :admin_auth]

    live_dashboard "/dashboard",
      metrics: PoddyclipBackendWeb.Telemetry,
      live_session_name: :admin_dashboard,
      additional_pages: [
        jobs: PoddyclipBackendWeb.Live.Admin.JobsPage,
        health: PoddyclipBackendWeb.Live.Admin.HealthPage
      ]
  end

  # Redirect root to dashboard
  scope "/" do
    pipe_through [:browser]

    get "/", PoddyclipBackendWeb.AdminRedirectController, :index
  end
end
