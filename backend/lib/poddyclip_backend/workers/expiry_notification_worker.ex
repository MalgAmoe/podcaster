defmodule PoddyclipBackend.Workers.ExpiryNotificationWorker do
  @moduledoc """
  Periodically checks for cancelled subscriptions that are about to expire
  and sends warning emails 7 days before the subscription ends.
  """
  use GenServer
  require Logger
  alias PoddyclipBackend.{Repo, Accounts, Accounts.User, Accounts.UserNotifier}
  import Ecto.Query

  # Check once per day
  @check_interval :timer.hours(24)
  # Send notification 7 days before expiry
  @warning_days 7

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @impl true
  def init(state) do
    # Initial check after a short delay to let the app start up
    Process.send_after(self(), :check_expiry, :timer.minutes(1))
    {:ok, state}
  end

  @impl true
  def handle_info(:check_expiry, state) do
    send_expiry_notifications()
    schedule_check()
    {:noreply, state}
  end

  defp schedule_check do
    Process.send_after(self(), :check_expiry, @check_interval)
  end

  defp send_expiry_notifications do
    now = DateTime.utc_now()
    warning_cutoff = DateTime.add(now, @warning_days, :day)

    # Find users with cancelled subscriptions that:
    # 1. End within the next 7 days
    # 2. Haven't already been notified (expiry_notification_sent_at is nil)
    users_to_notify =
      from(u in User,
        where: u.subscription_status == "cancelled",
        where: not is_nil(u.current_period_ends_at),
        where: u.current_period_ends_at > ^now,
        where: u.current_period_ends_at <= ^warning_cutoff,
        where: is_nil(u.expiry_notification_sent_at)
      )
      |> Repo.all()

    # Send notifications to eligible users
    Enum.each(users_to_notify, fn user ->
      if User.notification_enabled?(user, :subscription_expiry) do
        send_notification(user)
      end
    end)

    if length(users_to_notify) > 0 do
      Logger.info("Checked #{length(users_to_notify)} users for expiry notifications")
    end
  end

  defp send_notification(user) do
    days_remaining = calculate_days_remaining(user.current_period_ends_at)
    end_date = format_date(user.current_period_ends_at)

    try do
      UserNotifier.deliver_subscription_expiring(user, days_remaining, end_date)
      Accounts.record_expiry_notification(user)

      Logger.info("Expiry notification sent",
        user_id: user.id,
        days_remaining: days_remaining,
        end_date: end_date
      )
    rescue
      e ->
        Logger.error("Failed to send expiry notification",
          user_id: user.id,
          error: Exception.message(e)
        )
    end
  end

  defp calculate_days_remaining(period_end) do
    now = DateTime.utc_now()
    diff_seconds = DateTime.diff(period_end, now)
    max(1, div(diff_seconds, 86400))
  end

  defp format_date(datetime) do
    Calendar.strftime(datetime, "%B %d, %Y")
  end
end
