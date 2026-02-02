defmodule PoddyclipBackendWeb.Plugs.BasicAuth do
  @moduledoc """
  Simple Basic HTTP Auth plug for protecting the site during testing.

  Enable by setting environment variables:
    SITE_PASSWORD=mysecretpassword
    SITE_USERNAME=admin  (optional, defaults to "admin")

  The /health endpoint is always allowed through for k8s probes.
  """

  import Plug.Conn

  def init(opts), do: opts

  # Allow health checks and internal API (webhooks from Rust API)
  def call(%{request_path: "/health"} = conn, _opts), do: conn
  def call(%{request_path: "/api/internal/" <> _} = conn, _opts), do: conn
  def call(%{request_path: "/api/webhooks/" <> _} = conn, _opts), do: conn

  def call(conn, _opts) do
    case Application.get_env(:poddyclip_backend, :site_password) do
      nil -> conn
      "" -> conn
      password ->
        username = Application.get_env(:poddyclip_backend, :site_username, "admin")
        Plug.BasicAuth.basic_auth(conn, username: username, password: password)
    end
  end
end
