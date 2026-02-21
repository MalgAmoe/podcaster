defmodule PoddyclipBackend.Workers.FreePlanResetWorkerTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Workers.FreePlanResetWorker
  alias PoddyclipBackend.Billing

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  defp create_free_user(_context) do
    free_plan = free_plan_fixture()
    user = user_fixture()

    {:ok, user} =
      user
      |> Ecto.Changeset.change(
        plan_id: free_plan.id,
        seconds_available: 900,
        subscription_status: "none",
        current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)
      )
      |> Repo.update()

    %{user: user, free_plan: free_plan}
  end

  describe "reset_free_plans/0" do
    setup :create_free_user

    test "resets seconds for free user whose period has expired", %{user: user} do
      # Expire the period and use up seconds
      past = DateTime.utc_now() |> DateTime.add(-1, :hour) |> DateTime.truncate(:second)

      {:ok, user} =
        user
        |> Ecto.Changeset.change(
          current_period_ends_at: past,
          seconds_available: 50
        )
        |> Repo.update()

      assert user.seconds_available == 50

      FreePlanResetWorker.reset_free_plans()

      updated = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated.seconds_available == 900
      assert DateTime.compare(updated.current_period_ends_at, DateTime.utc_now()) == :gt
    end

    test "does not reset free user whose period has not expired", %{user: user} do
      # Period is still in the future, seconds partially used
      {:ok, user} =
        user
        |> Ecto.Changeset.change(seconds_available: 300)
        |> Repo.update()

      FreePlanResetWorker.reset_free_plans()

      updated = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated.seconds_available == 300
    end

    test "does not reset paid subscribers", %{user: user} do
      # Make user a paid subscriber with expired period
      past = DateTime.utc_now() |> DateTime.add(-1, :hour) |> DateTime.truncate(:second)

      {:ok, user} =
        user
        |> Ecto.Changeset.change(
          subscription_status: "active",
          current_period_ends_at: past,
          seconds_available: 100
        )
        |> Repo.update()

      FreePlanResetWorker.reset_free_plans()

      updated = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated.seconds_available == 100
    end

    test "does not reset user with nil period", %{user: user} do
      {:ok, user} =
        user
        |> Ecto.Changeset.change(
          current_period_ends_at: nil,
          seconds_available: 0
        )
        |> Repo.update()

      FreePlanResetWorker.reset_free_plans()

      updated = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated.seconds_available == 0
    end

    test "sets new period ~30 days in the future", %{user: user} do
      past = DateTime.utc_now() |> DateTime.add(-2, :day) |> DateTime.truncate(:second)

      {:ok, _user} =
        user
        |> Ecto.Changeset.change(current_period_ends_at: past, seconds_available: 0)
        |> Repo.update()

      FreePlanResetWorker.reset_free_plans()

      updated = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      expected = DateTime.add(DateTime.utc_now(), 30, :day)
      diff = DateTime.diff(updated.current_period_ends_at, expected, :second) |> abs()
      assert diff < 60
    end
  end

  describe "subscription downgrade sets free period" do
    test "expiry worker sets current_period_ends_at on downgrade" do
      munch_plan = munch_plan_fixture()
      user = user_fixture()

      past = DateTime.utc_now() |> DateTime.add(-1, :hour) |> DateTime.truncate(:second)

      {:ok, user} =
        user
        |> Ecto.Changeset.change(
          plan_id: munch_plan.id,
          subscription_status: "cancelled",
          polar_subscription_id: "sub_test",
          current_period_ends_at: past,
          seconds_available: 5000
        )
        |> Repo.update()

      {:ok, updated} = Billing.check_subscription_expiry(user)

      assert updated.subscription_status == "none"
      # Should have a new 30-day period, not nil
      assert updated.current_period_ends_at != nil
      assert DateTime.compare(updated.current_period_ends_at, DateTime.utc_now()) == :gt
    end
  end
end
