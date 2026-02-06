defmodule PoddyclipBackendWeb.UserSettingsController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackendWeb.LocaleHelpers

  # Helper for locale-aware redirects
  defp settings_path(conn) do
    locale = conn.assigns[:locale] || "en"
    LocaleHelpers.locale_path(locale, "/users/settings")
  end

  def confirm_email(conn, %{"token" => token}) do
    case Accounts.update_user_email(conn.assigns.current_scope.user, token) do
      {:ok, _user} ->
        conn
        |> put_flash(:info, "Email changed successfully.")
        |> redirect(to: settings_path(conn))

      {:error, _} ->
        conn
        |> put_flash(:error, "Email change link is invalid or it has expired.")
        |> redirect(to: settings_path(conn))
    end
  end
end
