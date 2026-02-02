defmodule PoddyclipBackend.Workers.SubscriptionExpiryWorker do
  @moduledoc """
  Periodically checks for cancelled subscriptions whose period has ended
  and downgrades them to free tier.
  """
  use GenServer
  require Logger
  alias PoddyclipBackend.{Repo, Accounts.User}
  import Ecto.Query

  # Check every hour
  @check_interval :timer.hours(1)

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @impl true
  def init(state) do
    schedule_check()
    {:ok, state}
  end

  @impl true
  def handle_info(:check_expiry, state) do
    expire_subscriptions()
    schedule_check()
    {:noreply, state}
  end

  defp schedule_check do
    Process.send_after(self(), :check_expiry, @check_interval)
  end

  defp expire_subscriptions do
    now = DateTime.utc_now()

    # Find users with cancelled subscriptions whose period has ended
    expired_users =
      from(u in User,
        where: u.subscription_status == "cancelled",
        where: not is_nil(u.current_period_ends_at),
        where: u.current_period_ends_at < ^now
      )
      |> Repo.all()

    for user <- expired_users do
      Logger.info("Expiring subscription for user #{user.id}")

      user
      |> Ecto.Changeset.change(%{
        subscription_status: "none",
        polar_subscription_id: nil,
        current_period_ends_at: nil,
        plan_id: nil,
        seconds_available: 900
      })
      |> Repo.update()
    end

    if length(expired_users) > 0 do
      Logger.info("Expired #{length(expired_users)} subscriptions")
    end
  end
end
