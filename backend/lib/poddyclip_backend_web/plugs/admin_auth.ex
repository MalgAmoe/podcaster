defmodule PoddyclipBackendWeb.Plugs.AdminAuth do
  @moduledoc """
  Plug for admin authentication using HTTP Basic Auth.

  Requires ADMIN_USERNAME and ADMIN_PASSWORD to be set in runtime config.
  """

  import Plug.Conn

  def init(opts), do: opts

  def call(conn, _opts) do
    username = Application.get_env(:poddyclip_backend, :admin_username)
    password = Application.get_env(:poddyclip_backend, :admin_password)

    if username && password do
      Plug.BasicAuth.basic_auth(conn, username: username, password: password)
    else
      conn
      |> put_resp_content_type("text/plain")
      |> send_resp(500, "Admin credentials not configured")
      |> halt()
    end
  end
end
