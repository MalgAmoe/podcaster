defmodule PoddyclipBackendWeb.Router do
  use PoddyclipBackendWeb, :router

  import PoddyclipBackendWeb.UserAuth
  import Plug.Conn

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

  # Public pages (no auth required)
  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser]

    get "/", PageController, :landing
    get "/pricing", PageController, :pricing
    get "/terms", PageController, :terms
    get "/privacy", PageController, :privacy
  end

  # Authenticated app
  scope "/app", PoddyclipBackendWeb do
    pipe_through [:browser, :require_authenticated_user]

    get "/", PageController, :process
  end

  # Internal API for Rust service webhooks
  scope "/api/internal", PoddyclipBackendWeb do
    pipe_through :api

    post "/jobs/:job_id/status", WebhookController, :job_status
  end

  # JSON API for frontend (session-authenticated)
  scope "/api", PoddyclipBackendWeb.Api do
    pipe_through :api_auth

    get "/presets", ProcessController, :presets
    post "/presign-upload", ProcessController, :presign_upload
    get "/jobs/current", ProcessController, :current_job
    post "/jobs", ProcessController, :create_job
    delete "/jobs/:id", ProcessController, :cancel_job
    get "/user", ProcessController, :current_user
  end

  # Enable LiveDashboard and Swoosh mailbox in development
  if Application.compile_env(:poddyclip_backend, :dev_routes) do
    # If you want to use the LiveDashboard in production, you should put
    # it behind authentication and allow only admins to access it.
    # If your application does not have an admins-only section yet,
    # you can use Plug.BasicAuth to set up some basic authentication
    # as long as you are also using SSL (which you should anyway).
    import Phoenix.LiveDashboard.Router

    scope "/dev" do
      pipe_through :browser

      live_dashboard "/dashboard", metrics: PoddyclipBackendWeb.Telemetry
      forward "/mailbox", Plug.Swoosh.MailboxPreview
    end
  end

  ## Authentication routes

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :redirect_if_user_is_authenticated]

    get "/users/register", UserRegistrationController, :new
    post "/users/register", UserRegistrationController, :create
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser, :require_authenticated_user]

    get "/users/settings", UserSettingsController, :edit
    put "/users/settings", UserSettingsController, :update
    get "/users/settings/confirm-email/:token", UserSettingsController, :confirm_email
  end

  scope "/", PoddyclipBackendWeb do
    pipe_through [:browser]

    get "/users/log-in", UserSessionController, :new
    get "/users/log-in/:token", UserSessionController, :confirm
    post "/users/log-in", UserSessionController, :create
    delete "/users/log-out", UserSessionController, :delete
  end
end
