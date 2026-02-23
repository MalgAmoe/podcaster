defmodule PoddyclipBackend.Workers.FreePlanResetWorker do
  @moduledoc """
  Periodically resets free plan users' seconds when their 30-day period ends.

  Free users get 900 seconds (15 min) per 30-day period. This worker checks
  every 6 hours for free users whose period has expired and resets their
  seconds_available to 900, starting a new 30-day period.
  """
  use GenServer
  require Logger
  alias PoddyclipBackend.{Repo, Admin, Accounts.User}
  import Ecto.Query

  @check_interval :timer.hours(6)

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @impl true
  def init(state) do
    schedule_check()
    {:ok, state}
  end

  @impl true
  def handle_info(:check_free_resets, state) do
    reset_free_plans()
    schedule_check()
    {:noreply, state}
  end

  defp schedule_check do
    Process.send_after(self(), :check_free_resets, @check_interval)
  end

  @doc """
  Finds free users whose 30-day period has ended and resets their seconds.
  Can be called manually for testing.
  """
  def reset_free_plans do
    now = DateTime.utc_now()
    free_plan = PoddyclipBackend.Billing.get_or_create_free_plan()

    expired_free_users =
      from(u in User,
        where: u.subscription_status == "none",
        where: not is_nil(u.current_period_ends_at),
        where: u.current_period_ends_at < ^now
      )
      |> Repo.all()

    for user <- expired_free_users do
      new_period_end = now |> DateTime.add(30, :day) |> DateTime.truncate(:second)

      user
      |> Ecto.Changeset.change(%{
        seconds_available: free_plan.seconds,
        current_period_ends_at: new_period_end
      })
      |> Repo.update()
      |> case do
        {:ok, _updated} ->
          Admin.log_billing_event("free_plan_reset", user.id, %{
            old_seconds: user.seconds_available,
            new_seconds: free_plan.seconds
          })

          Logger.info("Reset free plan seconds for user #{user.id}, next period ends #{new_period_end}")

        {:error, changeset} ->
          Logger.error("Failed to reset free plan for user #{user.id}: #{inspect(changeset.errors)}")
      end
    end

    if length(expired_free_users) > 0 do
      Logger.info("Reset #{length(expired_free_users)} free plan users")
    end
  end
end
