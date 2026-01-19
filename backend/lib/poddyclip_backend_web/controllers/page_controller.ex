defmodule PoddyclipBackendWeb.PageController do
  use PoddyclipBackendWeb, :controller

  # Public pages
  def landing(conn, _params) do
    conn
    |> put_layout(false)
    |> render(:landing)
  end

  def pricing(conn, _params) do
    conn
    |> put_layout(false)
    |> render(:pricing)
  end

  def terms(conn, _params) do
    conn
    |> put_layout(false)
    |> render(:terms)
  end

  def privacy(conn, _params) do
    conn
    |> put_layout(false)
    |> render(:privacy)
  end

  # Authenticated app
  def process(conn, _params) do
    # Use only root layout (no app layout) - root already has the navbar
    conn
    |> put_layout(false)
    |> render(:process)
  end
end
