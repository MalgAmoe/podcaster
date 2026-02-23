defmodule PoddyclipBackend.Workers.SubscriptionExpiryWorker do
  @moduledoc """
  Periodically checks for cancelled subscriptions whose period has ended
  and downgrades them to free tier.
  """
  use GenServer
  require Logger
  alias PoddyclipBackend.{Repo, Admin, Accounts.User}
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

  @doc """
  Finds cancelled subscriptions whose period has ended and downgrades to free.
  Can be called manually for testing.
  """
  def expire_subscriptions do
    now = DateTime.utc_now()
    free_plan = PoddyclipBackend.Billing.get_or_create_free_plan()

    # Find users with cancelled subscriptions whose period has ended
    expired_users =
      from(u in User,
        where: u.subscription_status == "cancelled",
        where: not is_nil(u.current_period_ends_at),
        where: u.current_period_ends_at < ^now,
        preload: [:plan]
      )
      |> Repo.all()

    for user <- expired_users do
      Logger.info("Expiring subscription for user #{user.id}")

      old_plan_name = if user.plan, do: user.plan.name, else: "unknown"
      old_seconds = user.seconds_available

      user
      |> Ecto.Changeset.change(%{
        subscription_status: "none",
        polar_subscription_id: nil,
        current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second),
        plan_id: free_plan.id,
        seconds_available: free_plan.seconds
      })
      |> Repo.update()
      |> case do
        {:ok, _updated} ->
          Admin.log_billing_event("subscription_downgraded", user.id, %{
            old_plan: old_plan_name,
            old_seconds: old_seconds,
            new_seconds: free_plan.seconds
          })

        {:error, changeset} ->
          Logger.error("Failed to expire subscription for user #{user.id}: #{inspect(changeset.errors)}")
      end
    end

    if length(expired_users) > 0 do
      Logger.info("Expired #{length(expired_users)} subscriptions")
    end
  end
end
