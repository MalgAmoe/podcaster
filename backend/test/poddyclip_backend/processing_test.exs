defmodule PoddyclipBackend.ProcessingTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Billing

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "minute refunds" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: plan.id,
          minutes_available: 10
        })

      %{user: user, plan: plan}
    end

    test "update_job_status/2 refunds minutes when job fails", %{user: user} do
      # Create a job with estimated_minutes
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_minutes: 5
        })
        |> Repo.insert!()

      # Update to failed
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed", "error" => "Test error"})

      # User should have minutes refunded (10 + 5 = 15)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 15
    end

    test "update_job_status/2 does not refund if job was already failed", %{user: user} do
      # Create a job that's already failed
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :failed,
          user_id: user.id,
          estimated_minutes: 5
        })
        |> Repo.insert!()

      # Update to failed again (webhook retry)
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed", "error" => "Test error"})

      # User should NOT get double refund
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 10
    end

    test "update_job_status/2 does not refund on completion", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_minutes: 5
        })
        |> Repo.insert!()

      # Update to completed
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "completed"})

      # User should NOT have minutes refunded
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 10
    end

    test "cancel_job/1 refunds minutes for active job", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_minutes: 3
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User should have minutes refunded (10 + 3 = 13)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 13
    end

    test "cancel_job/1 does not refund for already completed job", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :completed,
          user_id: user.id,
          estimated_minutes: 5
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User should NOT have minutes refunded
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 10
    end

    test "cancel_job/1 does not refund for already failed job", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :failed,
          user_id: user.id,
          estimated_minutes: 5
        })
        |> Repo.insert!()

      {:ok, _} = Processing.cancel_job(job.id)

      # User should NOT have minutes refunded (already refunded when it failed)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 10
    end

    test "handles job with nil estimated_minutes gracefully", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_minutes: nil
        })
        |> Repo.insert!()

      # Should not crash
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "failed"})

      # User minutes unchanged
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.minutes_available == 10
    end
  end
end
