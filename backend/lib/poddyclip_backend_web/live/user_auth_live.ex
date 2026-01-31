defmodule PoddyclipBackendWeb.UserAuthLive do
  @moduledoc """
  LiveView authentication hooks.

  - `:fetch_current_scope` - Mounts the current_scope to the socket assigns
  - `:require_authenticated_user` - Ensures the user is authenticated
  """
  import Phoenix.Component

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.Scope

  def on_mount(:fetch_current_scope, _params, session, socket) do
    socket = mount_current_scope(socket, session)
    {:cont, socket}
  end

  def on_mount(:require_authenticated_user, _params, session, socket) do
    socket = mount_current_scope(socket, session)

    if socket.assigns.current_scope && socket.assigns.current_scope.user do
      {:cont, socket}
    else
      # Get locale from session for locale-aware redirect
      locale = session["locale"] || "en"
      login_path = if locale == "es", do: "/es/users/log-in", else: "/users/log-in"

      socket =
        socket
        |> Phoenix.LiveView.put_flash(:error, "You must log in to access this page.")
        |> Phoenix.LiveView.redirect(to: login_path)

      {:halt, socket}
    end
  end

  defp mount_current_scope(socket, session) do
    case session do
      %{"user_token" => user_token} ->
        case Accounts.get_user_by_session_token(user_token) do
          {user, _token_inserted_at} ->
            assign(socket, :current_scope, Scope.for_user(user))

          nil ->
            assign(socket, :current_scope, nil)
        end

      _ ->
        assign(socket, :current_scope, nil)
    end
  end
end
