defmodule PoddyclipBackendWeb.Router do
  use PoddyclipBackendWeb, :router

  import PoddyclipBackendWeb.UserAuth
  import Plug.Conn

  # Health check for k8s probes (no pipeline, minimal overhead)
  scope "/health", PoddyclipBackendWeb do
    get "/", HealthController, :index
  end

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PoddyclipBackendWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
    plug :fetch_current_scope_for_user
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  # API pipeline with session-based auth (for React frontend)
  pipeline :api_auth do
    plug :accepts, ["json"]
    plug :fetch_session
    plug :fetch_current_scope_for_user
    plug :require_authenticated_api_user
  end

  # Locale pipelines - set locale assign before SetLocale plug
  pipeline :locale_en do
    plug :put_locale, "en"
  end

  pipeline :locale_es do
    plug :put_locale, "es"
  end

  pipeline :locale_it do
    plug :put_locale, "it"
  end

  pipeline :locale_fr do
    plug :put_locale, "fr"
  end

  defp put_locale(conn, locale) do
    conn
    |> assign(:locale, locale)
    |> PoddyclipBackendWeb.Plugs.SetLocale.call([])
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

  # ============================================================
  # Spanish routes (with /es prefix)
  # ============================================================

  # Public pages - Spanish
  scope "/es", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_es]

    get "/", PageController, :landing
    get "/pricing", PageController, :pricing
    get "/terms", PageController, :terms
    get "/privacy", PageController, :privacy
    get "/legal", PageController, :legal
    get "/help", PageController, :help
  end

  # Authenticated app - Spanish
  scope "/es/app", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_es, :require_authenticated_user]

    get "/", PageController, :process
    get "/*path", PageController, :process
  end

  # ============================================================
  # Italian routes (with /it prefix)
  # ============================================================

  # Public pages - Italian
  scope "/it", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_it]

    get "/", PageController, :landing
    get "/pricing", PageController, :pricing
    get "/terms", PageController, :terms
    get "/privacy", PageController, :privacy
    get "/legal", PageController, :legal
    get "/help", PageController, :help
  end

  # Authenticated app - Italian
  scope "/it/app", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_it, :require_authenticated_user]

    get "/", PageController, :process
    get "/*path", PageController, :process
  end

  # ============================================================
  # French routes (with /fr prefix)
  # ============================================================

  # Public pages - French
  scope "/fr", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_fr]

    get "/", PageController, :landing
    get "/pricing", PageController, :pricing
    get "/terms", PageController, :terms
    get "/privacy", PageController, :privacy
    get "/legal", PageController, :legal
    get "/help", PageController, :help
  end

  # Authenticated app - French
  scope "/fr/app", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_fr, :require_authenticated_user]

    get "/", PageController, :process
    get "/*path", PageController, :process
  end

  # ============================================================
  # English routes (default, no prefix)
  # ============================================================

  # Public pages (no auth required)
  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_en]

    get "/", PageController, :landing
    get "/pricing", PageController, :pricing
    get "/terms", PageController, :terms
    get "/privacy", PageController, :privacy
    get "/legal", PageController, :legal
    get "/help", PageController, :help
  end

  # Authenticated app (SolidJS handles client-side routing for /app/*)
  scope "/app", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_en, :require_authenticated_user]

    get "/", PageController, :process
    get "/*path", PageController, :process
  end

  # Internal API for Rust service webhooks
  scope "/api/internal", PoddyclipBackendWeb do
    pipe_through :api

    post "/jobs/:job_id/status", WebhookController, :job_status
    get "/users/:user_id/check_seconds", WebhookController, :check_seconds
  end

  # Polar billing webhooks
  scope "/api/webhooks", PoddyclipBackendWeb do
    pipe_through :api

    post "/polar", PolarWebhookController, :handle
  end

  # JSON API for frontend (session-authenticated)
  scope "/api", PoddyclipBackendWeb.Api do
    pipe_through :api_auth

    get "/presets", ProcessController, :presets
    post "/presign-upload", ProcessController, :presign_upload
    get "/jobs/current", ProcessController, :current_job
    get "/jobs/history", ProcessController, :job_history
    post "/jobs", ProcessController, :create_job
    delete "/jobs/:id", ProcessController, :cancel_job
    post "/jobs/:id/dismiss", ProcessController, :dismiss_job
    get "/jobs/:id/download_url", ProcessController, :download_url
    get "/user", ProcessController, :current_user
  end

  # Admin routes are served on a separate endpoint (AdminEndpoint on port 4001)
  # Access via SSH tunnel: ssh -L 4001:localhost:4001 user@server
  # Then open http://localhost:4001/admin/dashboard

  # Enable LiveDashboard and Swoosh mailbox in development
  if Application.compile_env(:poddyclip_backend, :dev_routes) do
    import Phoenix.LiveDashboard.Router

    scope "/dev" do
      pipe_through :browser

      live_dashboard "/dashboard",
        metrics: PoddyclipBackendWeb.Telemetry,
        live_session_name: :dev_dashboard
      forward "/mailbox", Plug.Swoosh.MailboxPreview
    end
  end

  ## Authentication routes

  # ============================================================
  # Spanish authentication routes
  # ============================================================

  scope "/es", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_es, :redirect_if_user_is_authenticated]

    get "/users/register", UserSessionController, :redirect_to_login
  end

  scope "/es", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_es, :require_authenticated_user]

    live_session :authenticated_es,
      on_mount: [
        {PoddyclipBackendWeb.UserAuthLive, :require_authenticated_user},
        {PoddyclipBackendWeb.LocaleHook, :set_locale}
      ] do
      live "/account", AccountLive
      live "/users/settings", SettingsLive
    end

    get "/users/settings/confirm-email/:token", UserSettingsController, :confirm_email
  end

  scope "/es", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_es]

    get "/users/log-in", UserSessionController, :new
    get "/users/log-in/:token", UserSessionController, :confirm
    post "/users/log-in", UserSessionController, :create
    delete "/users/log-out", UserSessionController, :delete
  end

  # ============================================================
  # Italian authentication routes
  # ============================================================

  scope "/it", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_it, :redirect_if_user_is_authenticated]

    get "/users/register", UserSessionController, :redirect_to_login
  end

  scope "/it", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_it, :require_authenticated_user]

    live_session :authenticated_it,
      on_mount: [
        {PoddyclipBackendWeb.UserAuthLive, :require_authenticated_user},
        {PoddyclipBackendWeb.LocaleHook, :set_locale}
      ] do
      live "/account", AccountLive
      live "/users/settings", SettingsLive
    end

    get "/users/settings/confirm-email/:token", UserSettingsController, :confirm_email
  end

  scope "/it", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_it]

    get "/users/log-in", UserSessionController, :new
    get "/users/log-in/:token", UserSessionController, :confirm
    post "/users/log-in", UserSessionController, :create
    delete "/users/log-out", UserSessionController, :delete
  end

  # ============================================================
  # French authentication routes
  # ============================================================

  scope "/fr", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_fr, :redirect_if_user_is_authenticated]

    get "/users/register", UserSessionController, :redirect_to_login
  end

  scope "/fr", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_fr, :require_authenticated_user]

    live_session :authenticated_fr,
      on_mount: [
        {PoddyclipBackendWeb.UserAuthLive, :require_authenticated_user},
        {PoddyclipBackendWeb.LocaleHook, :set_locale}
      ] do
      live "/account", AccountLive
      live "/users/settings", SettingsLive
    end

    get "/users/settings/confirm-email/:token", UserSettingsController, :confirm_email
  end

  scope "/fr", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_fr]

    get "/users/log-in", UserSessionController, :new
    get "/users/log-in/:token", UserSessionController, :confirm
    post "/users/log-in", UserSessionController, :create
    delete "/users/log-out", UserSessionController, :delete
  end

  # ============================================================
  # English authentication routes (default)
  # ============================================================

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_en, :redirect_if_user_is_authenticated]

    # Redirect old register URLs to sign-in (consolidated flow)
    get "/users/register", UserSessionController, :redirect_to_login
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_en, :require_authenticated_user]

    live_session :authenticated,
      on_mount: [
        {PoddyclipBackendWeb.UserAuthLive, :require_authenticated_user},
        {PoddyclipBackendWeb.LocaleHook, :set_locale}
      ] do
      live "/account", AccountLive
      live "/users/settings", SettingsLive
    end

    get "/users/settings/confirm-email/:token", UserSettingsController, :confirm_email
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :locale_en]

    get "/users/log-in", UserSessionController, :new
    get "/users/log-in/:token", UserSessionController, :confirm
    post "/users/log-in", UserSessionController, :create
    delete "/users/log-out", UserSessionController, :delete
  end
end
