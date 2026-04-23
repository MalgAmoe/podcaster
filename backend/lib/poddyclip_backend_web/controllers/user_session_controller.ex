defmodule PoddyclipBackendWeb.UserSessionController do
  @moduledoc """
  Magic-link authentication: request link, confirm, log in, log out.
  Merges guest sessions into real accounts on first login.
  """
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackendWeb.UserAuth

  def new(conn, _params) do
    conn = maybe_store_return_to_from_params(conn, conn.params)
    user = get_in(conn.assigns, [:current_scope, Access.key(:user)])
    email = if user && !user.is_guest, do: user.email, else: nil
    form = Phoenix.Component.to_form(%{"email" => email}, as: "user")
    return_to = get_session(conn, :user_return_to)

    conn
    |> assign(:conn, conn)
    |> render(:new, form: form, return_to: return_to)
  end

  # magic link login
  def create(conn, %{"user" => %{"token" => token} = user_params} = params) do
    conn = maybe_store_return_to_from_params(conn, params)
    info =
      case params do
        %{"_action" => "confirmed"} -> "User confirmed successfully."
        _ -> "Welcome back!"
      end

    case Accounts.login_user_by_magic_link(token) do
      {:ok, {user, _expired_tokens}} ->
        # Merge guest jobs into this account if logging in from a guest session
        guest_id = get_session(conn, :guest_user_id)

        if guest_id && guest_id != user.id do
          Accounts.merge_guest_into_user(guest_id, user.id)
        end

        conn
        |> delete_session(:guest_user_id)
        |> put_flash(:info, info)
        |> UserAuth.log_in_user(user, user_params)

      {:error, :not_found} ->
        conn
        |> put_flash(:error, "The link is invalid or it has expired.")
        |> assign(:conn, conn)
        |> render(:new, form: Phoenix.Component.to_form(%{}, as: "user"), return_to: get_session(conn, :user_return_to))
    end
  end

  # magic link request - auto-creates user if not found
  def create(conn, %{"user" => %{"email" => email}}) do
    conn = maybe_store_return_to_from_params(conn, conn.params)
    user = Accounts.get_user_by_email(email) || create_user_for_email(email)
    return_to = get_session(conn, :user_return_to)

    if user do
      Accounts.deliver_login_instructions(
        user,
        fn token ->
          base_url = PoddyclipBackendWeb.Endpoint.url()
          login_url = base_url <> "/users/log-in/#{token}"

          if return_to do
            login_url <> "?return_to=" <> URI.encode_www_form(return_to)
          else
            login_url
          end
        end
      )
    end

    # Same message whether new or existing (prevents email enumeration)
    conn
    |> put_flash(:info, "Check your email for a sign-in link. (Check spam if you don't see it)")
    |> redirect(to: "/users/log-in")
  end

  defp create_user_for_email(email) do
    case Accounts.register_user(%{email: email}) do
      {:ok, user} -> user
      {:error, _changeset} -> nil
    end
  end

  def confirm(conn, %{"token" => token}) do
    conn = maybe_store_return_to_from_params(conn, conn.params)

    if user = Accounts.get_user_by_magic_link_token(token) do
      form = Phoenix.Component.to_form(%{"token" => token}, as: "user")
      return_to = get_session(conn, :user_return_to)

      conn
      |> assign(:user, user)
      |> assign(:form, form)
      |> assign(:return_to, return_to)
      |> assign(:conn, conn)
      |> render(:confirm)
    else
      conn
      |> put_flash(:error, "Magic link is invalid or it has expired.")
      |> redirect(to: "/users/log-in")
    end
  end

  def delete(conn, _params) do
    conn
    |> put_flash(:info, "Logged out successfully.")
    |> UserAuth.log_out_user()
  end

  defp maybe_store_return_to_from_params(conn, %{"return_to" => return_to}) do
    if safe_return_to?(return_to) do
      put_session(conn, :user_return_to, return_to)
    else
      conn
    end
  end

  defp maybe_store_return_to_from_params(conn, _params), do: conn

  defp safe_return_to?(path) when is_binary(path) do
    String.starts_with?(path, "/") and
      not String.starts_with?(path, "//") and
      not String.contains?(path, "://")
  end

  defp safe_return_to?(_), do: false
end
