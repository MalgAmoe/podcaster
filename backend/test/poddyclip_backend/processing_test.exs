defmodule PoddyclipBackend.ProcessingTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Billing

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "seconds refunds" do
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

    test "update_job_status/2 refunds seconds when job fails", %{user: user} do
      # Create a job with estimated_seconds
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

      # User should have seconds refunded (600 + 300 = 900)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 900
    end

    test "update_job_status/2 does not refund if job was already failed", %{user: user} do
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

      # User should NOT get double refund
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "update_job_status/2 does not refund on completion", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "test.mp3",
          status: :processing,
          user_id: user.id,
          estimated_seconds: 300
        })
        |> Repo.insert!()

      # Update to completed
      {:ok, _job} = Processing.update_job_status(job.id, %{"status" => "completed"})

      # User should NOT have seconds refunded
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "cancel_job/1 refunds seconds for active job", %{user: user} do
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

      # User should have seconds refunded (600 + 180 = 780)
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 780
    end

    test "cancel_job/1 does not refund for already completed job", %{user: user} do
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

      # User should NOT have seconds refunded
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 600
    end

    test "cancel_job/1 does not refund for already failed job", %{user: user} do
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

      # User should NOT have seconds refunded (already refunded when it failed)
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
end
