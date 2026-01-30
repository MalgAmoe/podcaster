defmodule PoddyclipBackendWeb.HealthController do
  use PoddyclipBackendWeb, :controller

  def index(conn, _params) do
    json(conn, %{status: "ok"})
  end
end
