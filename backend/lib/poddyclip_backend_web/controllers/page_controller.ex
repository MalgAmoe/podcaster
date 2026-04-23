defmodule PoddyclipBackendWeb.PageController do
  @moduledoc """
  Public pages: landing page, `/app` (SolidJS mount), `/feedback`,
  `/help`, plus parked legal pages (terms/privacy/legal) kept for later
  reintroduction.
  """
  use PoddyclipBackendWeb, :controller

  def landing(conn, _params) do
    conn = PoddyclipBackendWeb.Plugs.EnsureGuestUser.call(conn, [])

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> assign(:page_title, "Munchy Cow | Clean Up Voice Recordings Online")
    |> assign(
      :meta_description,
      "Clean up spoken audio online with a free 20-second preview. Reduce noise, tame harshness, and level voice recordings for clearer listening."
    )
    |> render(:landing)
  end

  # Main app - ensure guest user exists, serve SolidJS app
  def app(conn, _params) do
    user = conn.assigns[:current_scope] && conn.assigns.current_scope.user

    if user && !user.is_guest do
      conn
      |> put_layout(false)
      |> assign(:conn, conn)
      |> render(:process)
    else
      redirect(conn, to: "/")
    end
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
end
