defmodule PoddyclipBackendWeb.AdminRedirectController do
  @moduledoc """
  Redirects bare `/` on the admin endpoint (port 4001) to `/admin/dashboard`.
  """
  use PoddyclipBackendWeb, :controller

  def index(conn, _params) do
    redirect(conn, to: "/admin/dashboard")
  end
end
