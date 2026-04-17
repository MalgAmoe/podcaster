defmodule PoddyclipBackendWeb.PageController do
  @moduledoc """
  Public pages: landing redirect, `/app` (SolidJS mount), `/feedback`,
  `/help`, plus parked legal pages (terms/privacy/legal) kept for later
  reintroduction.
  """
  use PoddyclipBackendWeb, :controller

  # Main app - ensure guest user exists, serve SolidJS app
  def app(conn, _params) do
    conn = PoddyclipBackendWeb.Plugs.EnsureGuestUser.call(conn, [])

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:process)
  end

  def redirect_to_app(conn, _params) do
    redirect(conn, to: "/app")
  end

  def terms(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:terms)
  end

  def privacy(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:privacy)
  end

  def help(conn, _params) do
    conn
    |> assign(:conn, conn)
    |> render(:help)
  end

  def feedback(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> assign(:submitted, false)
    |> render(:feedback)
  end

  def submit_feedback(conn, %{"value" => value}) do
    user_id = conn.assigns.current_scope.user.id

    if String.trim(value) != "" do
      PoddyclipBackend.Feedback.create_feedback(%{
        user_id: user_id,
        prompt_key: "open",
        value: String.trim(value)
      })
    end

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> assign(:submitted, true)
    |> render(:feedback)
  end

  def legal(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:legal)
  end

  # Authenticated app
  def process(conn, _params) do
    # Use only root layout (no app layout) - root already has the navbar
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:process)
  end

end
