defmodule PoddyclipBackendWeb.PageController do
  use PoddyclipBackendWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end
