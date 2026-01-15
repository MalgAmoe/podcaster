defmodule PoddyclipBackendWeb.PageController do
  use PoddyclipBackendWeb, :controller

  def process(conn, _params) do
    # Use only root layout (no app layout) - root already has the navbar
    conn
    |> put_layout(false)
    |> render(:process)
  end
end
