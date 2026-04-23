defmodule PoddyclipBackendWeb.SettingsLive do
  @moduledoc """
  `/users/settings` — email change, notification preferences, and
  account deletion. Requires sudo mode (recent authentication).
  """
  use PoddyclipBackendWeb, :live_view

  alias PoddyclipBackend.{Accounts, Billing}

  require Logger

  @impl true
  def mount(_params, _session, socket) do
    user = socket.assigns.current_scope.user

    # Check sudo mode - user must have authenticated recently
    if Accounts.sudo_mode?(user, -10) do
      socket =
        socket
        |> assign(
          user: user,
          email_changeset: Accounts.change_user_email(user),
          notification_preferences: user.notification_preferences || default_notification_prefs(),
          show_delete_modal: false,
          delete_confirmation: ""
        )

      {:ok, socket}
    else
      # Redirect to login if not in sudo mode
      socket =
        socket
        |> put_flash(:info, "You must re-authenticate to access this page.")
        |> redirect(to: "/users/log-in?return_to=/users/settings")

      {:ok, socket}
    end
  end

  defp default_notification_prefs do
    Map.new(PoddyclipBackend.Accounts.User.notification_types(), &{&1, true})
  end

  @impl true
  def handle_event("update_email", %{"user" => user_params}, socket) do
    user = socket.assigns.user
    case Accounts.change_user_email(user, user_params) do
      %{valid?: true} = changeset ->
        base_url = PoddyclipBackendWeb.Endpoint.url()

        confirm_url_fn = fn token ->
          "#{base_url}/users/settings/confirm-email/#{token}"
        end

        Accounts.deliver_user_update_email_instructions(
          Ecto.Changeset.apply_action!(changeset, :insert),
          user.email,
          confirm_url_fn
        )

        {:noreply,
         socket
         |> put_flash(:info, "A link to confirm your email change has been sent to the new address.")
         |> assign(email_changeset: Accounts.change_user_email(user))}

      changeset ->
        {:noreply, assign(socket, email_changeset: %{changeset | action: :insert})}
    end
  end

  @impl true
  def handle_event("update_notifications", params, socket) do
    user = socket.assigns.user
    notification_params = params["notifications"] || %{}

    new_prefs = %{
      "job_complete" => notification_params["job_complete"] == "true",
      "job_failed" => notification_params["job_failed"] == "true",
      "low_time" => notification_params["low_time"] == "true",
      "subscription_expiry" => notification_params["subscription_expiry"] == "true"
    }

    case Accounts.update_notification_preferences(user, new_prefs) do
      {:ok, updated_user} ->
        {:noreply,
         socket
         |> put_flash(:info, "Notification preferences updated.")
         |> assign(user: updated_user, notification_preferences: new_prefs)}

      {:error, _changeset} ->
        {:noreply, put_flash(socket, :error, "Failed to update notification preferences.")}
    end
  end

  @impl true
  def handle_event("show_delete_modal", _params, socket) do
    user = socket.assigns.user
    deletion_info = compute_deletion_info(user)

    {:noreply,
     socket
     |> assign(show_delete_modal: true, delete_confirmation: "")
     |> assign(deletion_info: deletion_info)}
  end

  @impl true
  def handle_event("noop", _params, socket) do
    # Captures click to prevent event bubbling to backdrop
    {:noreply, socket}
  end

  @impl true
  def handle_event("hide_delete_modal", _params, socket) do
    {:noreply, assign(socket, show_delete_modal: false, delete_confirmation: "")}
  end

  @impl true
  def handle_event("update_delete_confirmation", %{"value" => value}, socket) do
    {:noreply, assign(socket, delete_confirmation: value)}
  end

  @impl true
  def handle_event("confirm_delete_account", _params, socket) do
    user = socket.assigns.user

    if socket.assigns.delete_confirmation == "DELETE" do
      case Accounts.delete_user_account(user) do
        {:ok, _deleted_user} ->
          Logger.info("User account deleted via settings", user_id: user.id, email: user.email)

          {:noreply,
           socket
           |> put_flash(:info, "Your account has been permanently deleted.")
           |> redirect(to: "/")}

        {:error, _reason} ->
          {:noreply,
           socket
           |> assign(show_delete_modal: false, delete_confirmation: "")
           |> put_flash(:error, "Failed to delete account. Please contact support.")}
      end
    else
      {:noreply, socket}
    end
  end

  # Computes what the user will lose on account deletion
  defp compute_deletion_info(user) do
    pack_summary = Billing.get_pack_summary(user.id)

    subscription_info =
      if user.subscription_status == "active" && user.current_period_ends_at do
        days_remaining = Date.diff(DateTime.to_date(user.current_period_ends_at), Date.utc_today())
        %{
          active: true,
          days_remaining: max(0, days_remaining),
          ends_at: user.current_period_ends_at
        }
      else
        %{active: false, days_remaining: 0, ends_at: nil}
      end

    %{
      subscription: subscription_info,
      snacks: %{
        count: pack_summary.pack_count,
        minutes: div(pack_summary.total_seconds, 60)
      },
      has_anything_to_lose: subscription_info.active || pack_summary.pack_count > 0
    }
  end
end
