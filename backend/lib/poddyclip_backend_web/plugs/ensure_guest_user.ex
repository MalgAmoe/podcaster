defmodule PoddyclipBackendWeb.Plugs.EnsureGuestUser do
  @moduledoc """
  Plug that ensures a user exists in the session.
  If no user is logged in, auto-creates a guest user and logs them in.
  This allows the app to work without requiring signup.
  """

  import Plug.Conn

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.Scope

  def init(opts), do: opts

  def call(conn, _opts) do
    if conn.assigns[:current_scope] && conn.assigns.current_scope.user do
      # Already logged in (real user or existing guest), nothing to do
      conn
    else
      # No user in session, create a guest
      case Accounts.create_guest_user() do
        {:ok, guest} ->
          token = Accounts.generate_user_session_token(guest)

          conn
          |> put_session(:user_token, token)
          |> put_session(:guest_user_id, guest.id)
          |> assign(:current_scope, Scope.for_user(guest))

        {:error, _} ->
          # If guest creation fails, let the app render without a user
          conn
      end
    end
  end
end
