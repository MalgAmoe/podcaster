defmodule PoddyclipBackendWeb.Router do
  use PoddyclipBackendWeb, :router

  import PoddyclipBackendWeb.UserAuth
  import Plug.Conn

  # Health check for k8s probes (no pipeline, minimal overhead)
  scope "/health", PoddyclipBackendWeb do
    get("/", HealthController, :index)
  end

  # Sitemap for search engines
  scope "/", PoddyclipBackendWeb do
    get("/sitemap.xml", SitemapController, :index)
  end

  pipeline :browser do
    plug(:accepts, ["html"])
    plug(:fetch_session)
    plug(:fetch_live_flash)
    plug(:put_root_layout, html: {PoddyclipBackendWeb.Layouts, :root})
    plug(:protect_from_forgery)
    plug(:put_secure_browser_headers)
    plug(:fetch_current_scope_for_user)
  end

  pipeline :api do
    plug(:accepts, ["json"])
  end

  # API pipeline for the SolidJS app.
  # Also ensures guest users exist before enforcing authenticated API access.
  pipeline :api_auth do
    plug(:accepts, ["json"])
    plug(:fetch_session)
    plug(:fetch_current_scope_for_user)
    plug(PoddyclipBackendWeb.Plugs.EnsureGuestUser)
    plug(:require_authenticated_api_user)
  end

  defp require_authenticated_api_user(conn, _opts) do
    if conn.assigns[:current_scope] && conn.assigns.current_scope.user do
      assign(conn, :current_user, conn.assigns.current_scope.user)
    else
      conn
      |> put_status(:unauthorized)
      |> Phoenix.Controller.json(%{error: "Unauthorized"})
      |> halt()
    end
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through(:browser)

    get("/", PageController, :redirect_to_app)
    # PARKED — legal pages are intentionally kept in the codebase for later
    # reintroduction, but they are not part of the current public UI.
    # Keep the controller actions/templates in place, but do not expose the
    # routes or link to them elsewhere until that work is resumed.
    # get "/terms", PageController, :terms
    # get "/privacy", PageController, :privacy
    # get "/legal", PageController, :legal
    get("/help", PageController, :help)
  end

  scope "/app", PoddyclipBackendWeb do
    pipe_through([:browser, :require_non_guest_user])
    get("/past-munchings", PageController, :app)
  end

  scope "/app", PoddyclipBackendWeb do
    pipe_through(:browser)
    get("/", PageController, :app)
    get("/*path", PageController, :app)
  end

  # Internal API for Rust service webhooks
  scope "/api/internal", PoddyclipBackendWeb do
    pipe_through(:api)

    post("/jobs/:job_id/status", WebhookController, :job_status)
    get("/users/:user_id/check_seconds", WebhookController, :check_seconds)
  end

  # Polar billing webhooks
  scope "/api/webhooks", PoddyclipBackendWeb do
    pipe_through(:api)

    post("/polar", PolarWebhookController, :handle)
  end

  # JSON API for frontend (session-authenticated)
  scope "/api", PoddyclipBackendWeb.Api do
    pipe_through(:api_auth)

    post("/presign-upload", ProcessController, :presign_upload)
    get("/jobs/current", ProcessController, :current_job)
    get("/jobs/history", ProcessController, :job_history)
    post("/jobs", ProcessController, :create_job)
    delete("/jobs/:id", ProcessController, :cancel_job)
    post("/jobs/:id/dismiss", ProcessController, :dismiss_job)
    get("/jobs/:id/download_url", ProcessController, :download_url)
    get("/user", ProcessController, :current_user)
  end

  # Admin routes are served on a separate endpoint (AdminEndpoint on port 4001)
  # Access via SSH tunnel: ssh -L 4001:localhost:4001 user@server
  # Then open http://localhost:4001/admin/dashboard

  # Enable LiveDashboard and Swoosh mailbox in development
  if Application.compile_env(:poddyclip_backend, :dev_routes) do
    import Phoenix.LiveDashboard.Router

    scope "/dev" do
      pipe_through(:browser)

      live_dashboard("/dashboard",
        metrics: PoddyclipBackendWeb.Telemetry,
        live_session_name: :dev_dashboard
      )

      forward("/mailbox", Plug.Swoosh.MailboxPreview)
    end
  end

  ## Authentication routes

  scope "/", PoddyclipBackendWeb do
    pipe_through([:browser, :redirect_if_user_is_authenticated])

    # PARKED — separate registration remains preserved in code for later
    # reintroduction, but the current auth product flow is unified sign-in.
    # Keep /users/register as a redirect so old links keep working without
    # exposing the parked UI.
    get("/users/register", UserSessionController, :redirect_to_login)
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through([:browser, :require_non_guest_user])

    get("/feedback", PageController, :feedback)
    post("/feedback", PageController, :submit_feedback)
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through([:browser, :require_authenticated_user])

    live_session :authenticated,
      on_mount: [
        {PoddyclipBackendWeb.UserAuthLive, :require_authenticated_user}
      ] do
      live("/account", AccountLive)
      live("/users/settings", SettingsLive)
    end

    get("/users/settings/confirm-email/:token", UserSettingsController, :confirm_email)
    get("/users/settings/export-data", UserSettingsController, :export_data)
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through(:browser)

    get("/users/log-in", UserSessionController, :new)
    get("/users/log-in/:token", UserSessionController, :confirm)
    post("/users/log-in", UserSessionController, :create)
    delete("/users/log-out", UserSessionController, :delete)
  end
end
