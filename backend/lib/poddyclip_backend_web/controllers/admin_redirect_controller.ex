defmodule PoddyclipBackendWeb.AdminRedirectController do
  use PoddyclipBackendWeb, :controller

  def index(conn, _params) do
    redirect(conn, to: "/admin/dashboard")
  end
end
