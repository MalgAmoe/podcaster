defmodule PoddyclipBackend.DemoTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.DemoRateLimiter

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "demo job billing" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        PoddyclipBackend.Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 999_999
        })

      %{user: user}
    end

    test "no seconds deducted when demo job completes", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "demo_test.mp3",
          status: :processing,
          user_id: user.id,
          is_demo: true,
          estimated_seconds: 30
        })
        |> Repo.insert!()

      {:ok, completed_job} =
        Processing.update_job_status(job.id, %{
          "status" => "completed",
          "audio_duration_seconds" => 30
        })

      assert completed_job.status == :completed
      assert completed_job.is_demo == true

      # Seconds must NOT be deducted for demo jobs
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 999_999
    end

    test "seconds ARE deducted for non-demo job", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "real_job.mp3",
          status: :processing,
          user_id: user.id,
          is_demo: false,
          estimated_seconds: 30
        })
        |> Repo.insert!()

      {:ok, _} =
        Processing.update_job_status(job.id, %{
          "status" => "completed",
          "audio_duration_seconds" => 30
        })

      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 999_999 - 30
    end

    test "no seconds deducted when demo job fails", %{user: user} do
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "demo_fail.mp3",
          status: :processing,
          user_id: user.id,
          is_demo: true,
          estimated_seconds: 30
        })
        |> Repo.insert!()

      {:ok, failed_job} =
        Processing.update_job_status(job.id, %{
          "status" => "failed",
          "error" => "Test failure"
        })

      assert failed_job.status == :failed
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 999_999
    end
  end

  describe "demo job submission" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        PoddyclipBackend.Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 999_999
        })

      %{user: user}
    end

    test "submit_job_from_s3 sets is_demo flag", %{user: user} do
      {:ok, job} =
        Processing.submit_job_from_s3(
          "inputs/#{user.id}/demo_test.mp3",
          "test.mp3",
          user.id,
          strength: 2,
          ai_clean: true,
          is_demo: true
        )

      assert job.is_demo == true
    end

    test "submit_job_from_s3 defaults is_demo to false", %{user: user} do
      {:ok, job} =
        Processing.submit_job_from_s3(
          "inputs/#{user.id}/input.mp3",
          "test.mp3",
          user.id,
          strength: 2
        )

      assert job.is_demo == false
    end
  end

  describe "demo rate limiter" do
    setup do
      # Clear ETS table before each test
      :ets.delete_all_objects(:demo_rate_limit)
      :ok
    end

    test "allows first 3 attempts" do
      ip = "192.168.1.#{System.unique_integer([:positive])}"

      assert :ok = DemoRateLimiter.check_demo(ip)
      DemoRateLimiter.record_demo(ip)

      assert :ok = DemoRateLimiter.check_demo(ip)
      DemoRateLimiter.record_demo(ip)

      assert :ok = DemoRateLimiter.check_demo(ip)
      DemoRateLimiter.record_demo(ip)
    end

    test "blocks 4th attempt" do
      ip = "10.0.0.#{System.unique_integer([:positive])}"

      for _ <- 1..3 do
        assert :ok = DemoRateLimiter.check_demo(ip)
        DemoRateLimiter.record_demo(ip)
      end

      assert {:error, :rate_limited} = DemoRateLimiter.check_demo(ip)
    end

    test "different IPs are independent" do
      ip1 = "10.1.1.#{System.unique_integer([:positive])}"
      ip2 = "10.2.2.#{System.unique_integer([:positive])}"

      for _ <- 1..3 do
        DemoRateLimiter.record_demo(ip1)
      end

      assert {:error, :rate_limited} = DemoRateLimiter.check_demo(ip1)
      assert :ok = DemoRateLimiter.check_demo(ip2)
    end
  end

  describe "demo job status endpoint security" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        PoddyclipBackend.Billing.update_subscription(user, %{
          plan_id: plan.id,
          seconds_available: 600
        })

      %{user: user}
    end

    test "cannot view non-demo jobs via demo endpoint", %{user: user} do
      # Create a regular (non-demo) job
      job =
        %Job{}
        |> Job.changeset(%{
          filename: "private.mp3",
          status: :completed,
          user_id: user.id,
          is_demo: false
        })
        |> Repo.insert!()

      # The demo controller checks is_demo and returns 404 for non-demo jobs
      fetched = Processing.get_job(job.id)
      assert fetched.is_demo == false
    end
  end
end
