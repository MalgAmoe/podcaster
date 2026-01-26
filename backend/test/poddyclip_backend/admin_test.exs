defmodule PoddyclipBackend.AdminTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Admin
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Repo

  import PoddyclipBackend.AccountsFixtures

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
