defmodule PoddyclipBackendWeb.UserSettingsController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackendWeb.LocaleHelpers

  import PoddyclipBackendWeb.UserAuth, only: [require_sudo_mode: 2]

  # Helper for locale-aware redirects
  defp settings_path(conn) do
    locale = conn.assigns[:locale] || "en"
    LocaleHelpers.locale_path(locale, "/users/settings")
  end

  plug :require_sudo_mode
  plug :assign_email_changeset
  plug :assign_notification_preferences

  def edit(conn, _params) do
    render(conn, :edit)
  end

  def update(conn, %{"action" => "update_email"} = params) do
    %{"user" => user_params} = params
    user = conn.assigns.current_scope.user

    case Accounts.change_user_email(user, user_params) do
      %{valid?: true} = changeset ->
        Accounts.deliver_user_update_email_instructions(
          Ecto.Changeset.apply_action!(changeset, :insert),
          user.email,
          &url(~p"/users/settings/confirm-email/#{&1}")
        )

        conn
        |> put_flash(
          :info,
          "A link to confirm your email change has been sent to the new address."
        )
        |> redirect(to: settings_path(conn))

      changeset ->
        render(conn, :edit, email_changeset: %{changeset | action: :insert})
    end
  end

  def update(conn, %{"action" => "update_notifications"} = params) do
    user = conn.assigns.current_scope.user
    notification_params = params["notifications"] || %{}

    # Convert checkbox params to boolean map
    new_prefs = %{
      "job_complete" => notification_params["job_complete"] == "true",
      "job_failed" => notification_params["job_failed"] == "true",
      "low_minutes" => notification_params["low_minutes"] == "true",
      "subscription_expiry" => notification_params["subscription_expiry"] == "true"
    }

    case Accounts.update_notification_preferences(user, new_prefs) do
      {:ok, _user} ->
        conn
        |> put_flash(:info, "Notification preferences updated.")
        |> redirect(to: settings_path(conn))

      {:error, _changeset} ->
        conn
        |> put_flash(:error, "Failed to update notification preferences.")
        |> redirect(to: settings_path(conn))
    end
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

  defp assign_email_changeset(conn, _opts) do
    user = conn.assigns.current_scope.user
    assign(conn, :email_changeset, Accounts.change_user_email(user))
  end

  defp assign_notification_preferences(conn, _opts) do
    user = conn.assigns.current_scope.user
    prefs = user.notification_preferences || %{
      "job_complete" => true,
      "job_failed" => true,
      "low_minutes" => true,
      "subscription_expiry" => true
    }
    assign(conn, :notification_preferences, prefs)
  end
end
