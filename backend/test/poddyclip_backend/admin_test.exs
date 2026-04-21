defmodule PoddyclipBackend.AdminTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Admin
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Repo

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "job_stats/0" do
    test "returns correct counts for empty database" do
      stats = Admin.job_stats()

      assert stats.current.queued == 0
      assert stats.current.processing == 0
      assert stats.last_24h.completed == 0
      assert stats.last_24h.failed == 0
      assert stats.last_24h.success_rate == 100.0
    end

    test "counts jobs by status" do
      user = user_fixture()

      # Create jobs with different statuses
      insert_job(user.id, :queued)
      insert_job(user.id, :queued)
      insert_job(user.id, :processing)
      insert_job(user.id, :completed)
      insert_job(user.id, :completed)
      insert_job(user.id, :completed)
      insert_job(user.id, :failed)

      stats = Admin.job_stats()

      assert stats.current.queued == 2
      assert stats.current.processing == 1
      assert stats.last_24h.completed == 3
      assert stats.last_24h.failed == 1
      assert stats.last_24h.success_rate == 75.0
    end
  end

  describe "recent_errors/1" do
    test "returns empty list when no failures" do
      result = Admin.recent_errors()

      assert result.errors == []
      assert result.total_24h == 0
    end

    test "returns recent failed jobs" do
      user = user_fixture()

      job = insert_job(user.id, :failed, error: "Test error message")

      result = Admin.recent_errors()

      assert length(result.errors) == 1
      assert result.total_24h == 1

      [error] = result.errors
      assert error.job_id == job.id
      assert error.error == "Test error message"
      assert error.user_id == user.id
    end

    test "respects limit parameter" do
      user = user_fixture()

      for i <- 1..5 do
        insert_job(user.id, :failed, error: "Error #{i}")
      end

      result = Admin.recent_errors(3)

      assert length(result.errors) == 3
      assert result.total_24h == 5
    end
  end

  describe "active_jobs/0" do
    test "returns queued and processing jobs" do
      user = user_fixture()

      queued = insert_job(user.id, :queued)
      processing = insert_job(user.id, :processing)
      _completed = insert_job(user.id, :completed)
      _failed = insert_job(user.id, :failed)

      active = Admin.active_jobs()

      assert length(active) == 2
      job_ids = Enum.map(active, & &1.job_id)
      assert queued.id in job_ids
      assert processing.id in job_ids
    end
  end

  describe "user_stats/0" do
    test "counts total users" do
      _user1 = user_fixture()
      _user2 = user_fixture()

      stats = Admin.user_stats()

      assert stats.total == 2
    end

    test "groups users by subscription status" do
      user1 = user_fixture()
      user2 = user_fixture()

      # Update subscription statuses
      Repo.update!(Ecto.Changeset.change(user1, subscription_status: "active"))
      Repo.update!(Ecto.Changeset.change(user2, subscription_status: "none"))

      stats = Admin.user_stats()

      assert stats.by_subscription["active"] == 1
      assert stats.by_subscription["none"] == 1
    end
  end

  describe "health_check/0" do
    test "returns health status with database check" do
      health = Admin.health_check()

      assert health.status in ["healthy", "degraded", "unhealthy"]
      assert health.checks.database.status == "ok"
      assert is_integer(health.checks.database.latency_ms)
      assert %DateTime{} = health.timestamp
    end

    test "includes oban queue status" do
      health = Admin.health_check()

      assert health.checks.oban.status == "ok"
      assert is_map(health.checks.oban.queues)
    end
  end

  # ----- Billing Stats -----

  describe "log_billing_event/3" do
    test "inserts a billing event" do
      user = user_fixture()

      assert {:ok, event} =
               Admin.log_billing_event("free_plan_reset", user.id, %{
                 old_seconds: 0,
                 new_seconds: 1800
               })

      assert event.event_type == "free_plan_reset"
      assert event.user_id == user.id
      assert event.metadata[:old_seconds] == 0
      assert event.metadata[:new_seconds] == 1800
    end

    test "rejects invalid event types" do
      user = user_fixture()

      assert {:error, changeset} = Admin.log_billing_event("invalid_type", user.id, %{})
      assert errors_on(changeset).event_type
    end
  end

  describe "billing_stats/0" do
    test "returns all zero stats for empty database" do
      stats = Admin.billing_stats()

      assert stats.upcoming.free_expiring_24h == 0
      assert stats.upcoming.cancelled_expiring_24h == 0
      assert stats.upcoming.expired_free_pending == 0
      assert stats.upcoming.expired_cancelled_pending == 0
      assert stats.upcoming.expiry_notifications_due == 0

      assert stats.current_state.free_zero_seconds == 0
      assert stats.current_state.low_seconds_users == 0
      assert stats.current_state.past_due_subscriptions == 0
      assert stats.current_state.minute_packs_expiring_30d == 0
      assert stats.current_state.total_pack_seconds == 0

      assert stats.recent_activity.free_resets_24h == 0
      assert stats.recent_activity.downgrades_24h == 0
      assert stats.recent_activity.expiry_notifications_sent_7d == 0
    end

    test "counts expired free users pending reset" do
      free_plan = free_plan_fixture()
      user = user_fixture()

      # Expired free user (period ended yesterday)
      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "none",
          plan_id: free_plan.id,
          current_period_ends_at:
            DateTime.utc_now() |> DateTime.add(-1, :day) |> DateTime.truncate(:second)
        )
      )

      stats = Admin.billing_stats()
      assert stats.upcoming.expired_free_pending == 1
    end

    test "counts expired cancelled users pending downgrade" do
      munch_plan = munch_plan_fixture()
      user = user_fixture()

      # Expired cancelled user
      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "cancelled",
          plan_id: munch_plan.id,
          current_period_ends_at:
            DateTime.utc_now() |> DateTime.add(-1, :hour) |> DateTime.truncate(:second)
        )
      )

      stats = Admin.billing_stats()
      assert stats.upcoming.expired_cancelled_pending == 1
    end

    test "counts free users expiring in next 24h" do
      free_plan = free_plan_fixture()
      user = user_fixture()

      # Free user expiring in 12 hours
      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "none",
          plan_id: free_plan.id,
          current_period_ends_at:
            DateTime.utc_now() |> DateTime.add(12, :hour) |> DateTime.truncate(:second)
        )
      )

      stats = Admin.billing_stats()
      assert stats.upcoming.free_expiring_24h == 1
    end

    test "counts cancelled users expiring in next 24h" do
      munch_plan = munch_plan_fixture()
      user = user_fixture()

      # Cancelled user expiring in 6 hours
      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "cancelled",
          plan_id: munch_plan.id,
          current_period_ends_at:
            DateTime.utc_now() |> DateTime.add(6, :hour) |> DateTime.truncate(:second)
        )
      )

      stats = Admin.billing_stats()
      assert stats.upcoming.cancelled_expiring_24h == 1
    end

    test "counts expiry notifications due" do
      munch_plan = munch_plan_fixture()
      user = user_fixture()

      # Cancelled user expiring in 5 days, not yet notified
      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "cancelled",
          plan_id: munch_plan.id,
          current_period_ends_at:
            DateTime.utc_now() |> DateTime.add(5, :day) |> DateTime.truncate(:second),
          expiry_notification_sent_at: nil
        )
      )

      stats = Admin.billing_stats()
      assert stats.upcoming.expiry_notifications_due == 1
    end

    test "counts free users with zero seconds" do
      free_plan = free_plan_fixture()
      user = user_fixture()

      Repo.update!(
        Ecto.Changeset.change(user,
          subscription_status: "none",
          plan_id: free_plan.id,
          seconds_available: 0
        )
      )

      stats = Admin.billing_stats()
      assert stats.current_state.free_zero_seconds == 1
    end

    test "counts past_due subscriptions" do
      user = user_fixture()

      Repo.update!(Ecto.Changeset.change(user, subscription_status: "past_due"))

      stats = Admin.billing_stats()
      assert stats.current_state.past_due_subscriptions == 1
    end

    test "counts recent activity from audit log" do
      user = user_fixture()

      Admin.log_billing_event("free_plan_reset", user.id, %{})
      Admin.log_billing_event("free_plan_reset", user.id, %{})
      Admin.log_billing_event("subscription_downgraded", user.id, %{})
      Admin.log_billing_event("expiry_notification_sent", user.id, %{})

      stats = Admin.billing_stats()
      assert stats.recent_activity.free_resets_24h == 2
      assert stats.recent_activity.downgrades_24h == 1
      assert stats.recent_activity.expiry_notifications_sent_7d == 1
    end
  end

  # Helper to insert test jobs
  defp insert_job(user_id, status, opts \\ []) do
    %Job{}
    |> Job.changeset(%{
      user_id: user_id,
      filename: opts[:filename] || "test.mp3",
      status: status,
      error: opts[:error]
    })
    |> Repo.insert!()
  end
end
