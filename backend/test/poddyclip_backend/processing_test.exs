defmodule PoddyclipBackend.ProcessingTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Billing

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "billing on completion" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 600
        })

      %{user: user, plan: plan}
    end

    test "deducts actual seconds on completion", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: 100
        })
        |> Repo.insert!()

      # Complete with actual duration of 150s
      {:ok, completed_job} = Processing.update_job_status(job.id, %{
        "status" => "completed",
        "audio_duration_seconds" => 150
      })

      assert completed_job.actual_duration_seconds == 150

      # User should have actual seconds deducted (600 - 150 = 450)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 450
    end

    test "no deduction when actual duration not provided", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      # Complete without actual duration (legacy/fallback)
      {:ok, _} = Processing.update_job_status(job.id, %{
        "status" => "completed"
      })

      # No change - nothing to deduct without actual duration
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end
  end

  describe "no billing on failure" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 600
        })

      %{user: user, plan: plan}
    end

    test "no seconds change when job fails", %{user: user} do
      # Create a job (no seconds deducted upfront anymore)
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      # Update to failed
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed", "error" => "Test error"})

      # User seconds should be unchanged (nothing was deducted, nothing to refund)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "no seconds change on duplicate failure status", %{user: user} do
      # Create a job that's already failed
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :failed,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      # Update to failed again (webhook retry)
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed", "error" => "Test error"})

      # User seconds unchanged
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "handles job with nil estimated_seconds gracefully", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: nil
        })
        |> Repo.insert!()

      # Should not crash
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed"})

      # User seconds unchanged
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end
  end

  describe "no billing on cancel" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 600
        })

      %{user: user, plan: plan}
    end

    test "no seconds change when active job is cancelled", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: 180
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User seconds unchanged (nothing was deducted upfront)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "no seconds change when completed job is cleaned up", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :completed,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User seconds unchanged
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "no seconds change when failed job is cleaned up", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :failed,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User seconds unchanged
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end
  end
end
