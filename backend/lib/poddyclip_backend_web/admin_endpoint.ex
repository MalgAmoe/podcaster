defmodule PoddyclipBackendWeb.AdminEndpoint do
  @moduledoc """
  Separate endpoint for admin dashboard, listening on localhost only.

  Access via SSH tunnel:
    ssh -L 4001:localhost:4001 user@server
    Then open http://localhost:4001/admin/dashboard
  """

  use Phoenix.Endpoint, otp_app: :poddyclip_backend

  # The session will be stored in the cookie and signed
  @session_options [
    store: :cookie,
    key: "_poddyclip_admin_key",
    signing_salt: "AdminSalt1",
    same_site: "Lax"
  ]

  socket "/live", Phoenix.LiveView.Socket,
    websocket: [connect_info: [session: @session_options]],
    longpoll: [connect_info: [session: @session_options]]

  # Serve static files (needed for LiveDashboard assets)
  plug Plug.Static,
    at: "/",
    from: :poddyclip_backend,
    gzip: false,
    only: PoddyclipBackendWeb.static_paths()

  plug Phoenix.LiveDashboard.RequestLogger,
    param_key: "request_logger",
    cookie_key: "request_logger"

  plug Plug.RequestId
  plug Plug.Telemetry, event_prefix: [:phoenix, :admin_endpoint]

  plug Plug.Parsers,
    parsers: [:urlencoded, :multipart, :json],
    pass: ["*/*"],
    json_decoder: Phoenix.json_library()

  plug Plug.MethodOverride
  plug Plug.Head
  plug Plug.Session, @session_options
  plug PoddyclipBackendWeb.AdminRouter
end
