defmodule PoddyclipBackend.BillingTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.Billing

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "plans" do
    test "get_plan_by_name/1 returns the plan with given name" do
      plan = free_plan_fixture()
      assert Billing.get_plan_by_name("free").id == plan.id
    end

    test "get_plan_by_name/1 returns nil for non-existent plan" do
      assert Billing.get_plan_by_name("nonexistent") == nil
    end

    test "get_or_create_free_plan/0 creates free plan if not exists" do
      plan = Billing.get_or_create_free_plan()
      assert plan.name == "free"
      assert plan.seconds == 900
      assert plan.price_cents == 0
    end

    test "get_or_create_free_plan/0 returns existing plan" do
      existing = free_plan_fixture()
      plan = Billing.get_or_create_free_plan()
      assert plan.id == existing.id
    end

    test "list_active_plans/0 returns active plans ordered by price" do
      _free = free_plan_fixture()
      _pro = pro_plan_fixture()
      plans = Billing.list_active_plans()
      assert length(plans) >= 2
      # First should be free (price 0)
      assert hd(plans).price_cents == 0
    end
  end

  describe "seconds" do
    setup do
      plan = free_plan_fixture()
      user = user_fixture()
      # Update user with plan and seconds
      {:ok, user} =
        user
        |> Ecto.Changeset.change(plan_id: plan.id, seconds_available: 900)
        |> Repo.update()

      %{user: user, plan: plan}
    end

    test "has_seconds?/2 returns true when user has enough seconds", %{user: user} do
      assert Billing.has_seconds?(user, 300)
      assert Billing.has_seconds?(user, 900)
    end

    test "has_seconds?/2 returns false when user doesn't have enough", %{user: user} do
      refute Billing.has_seconds?(user, 901)
      refute Billing.has_seconds?(user, 6000)
    end

    test "deduct_seconds/2 subtracts from available seconds", %{user: user} do
      assert {:ok, updated} = Billing.deduct_seconds(user, 300)
      assert updated.seconds_available == 600
    end

    test "deduct_seconds/2 returns error when insufficient", %{user: user} do
      assert {:error, :insufficient_seconds} = Billing.deduct_seconds(user, 6000)
    end

    test "refund_seconds/2 adds to available seconds", %{user: user} do
      assert {:ok, updated} = Billing.refund_seconds(user, 300)
      assert updated.seconds_available == 1200
    end

    test "adjust_seconds/2 adjusts balance", %{user: user} do
      # Positive adjustment (refund)
      assert {:ok, updated} = Billing.adjust_seconds(user, 300)
      assert updated.seconds_available == 1200

      # Negative adjustment (deduct more)
      assert {:ok, updated2} = Billing.adjust_seconds(updated, -600)
      assert updated2.seconds_available == 600
    end

    test "adjust_seconds/2 doesn't go below zero", %{user: user} do
      assert {:ok, updated} = Billing.adjust_seconds(user, -6000)
      assert updated.seconds_available == 0
    end
  end

  describe "subscriptions" do
    setup do
      free_plan = free_plan_fixture()
      pro_plan = pro_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        user
        |> Ecto.Changeset.change(plan_id: free_plan.id, seconds_available: 900)
        |> Repo.update()

      %{user: user, free_plan: free_plan, pro_plan: pro_plan}
    end

    test "update_subscription/2 updates user billing fields", %{user: user, pro_plan: pro_plan} do
      period_end = DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)

      assert {:ok, updated} =
               Billing.update_subscription(user, %{
                 plan_id: pro_plan.id,
                 seconds_available: pro_plan.seconds,
                 subscription_status: "active",
                 polar_customer_id: "cus_123",
                 polar_subscription_id: "sub_456",
                 current_period_ends_at: period_end
               })

      assert updated.plan_id == pro_plan.id
      assert updated.seconds_available == 54000
      assert updated.subscription_status == "active"
      assert updated.polar_customer_id == "cus_123"
      assert updated.polar_subscription_id == "sub_456"
      assert DateTime.compare(updated.current_period_ends_at, period_end) == :eq
    end

    test "reset_subscription_seconds/1 resets to plan amount", %{user: user, pro_plan: pro_plan} do
      # First upgrade to pro
      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: pro_plan.id,
          seconds_available: 6000
        })

      # Now reset
      assert {:ok, updated} = Billing.reset_subscription_seconds(user)
      assert updated.seconds_available == 54000
    end

    test "get_user_by_subscription_id/1 finds user", %{user: user} do
      {:ok, _} = Billing.update_subscription(user, %{polar_subscription_id: "sub_test_123"})
      found = Billing.get_user_by_subscription_id("sub_test_123")
      assert found.id == user.id
    end

    test "get_user_by_customer_id/1 finds user", %{user: user} do
      {:ok, _} = Billing.update_subscription(user, %{polar_customer_id: "cus_test_456"})
      found = Billing.get_user_by_customer_id("cus_test_456")
      assert found.id == user.id
    end
  end

  describe "subscription expiry" do
    setup do
      free_plan = free_plan_fixture()
      pro_plan = pro_plan_fixture()
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: pro_plan.id,
          seconds_available: 30000,
          subscription_status: "cancelled",
          polar_subscription_id: "sub_expiry_test"
        })

      %{user: user, free_plan: free_plan, pro_plan: pro_plan}
    end

    test "check_subscription_expiry/1 downgrades expired cancelled subscription", %{user: user, free_plan: free_plan} do
      # Set period end in the past
      past = DateTime.utc_now() |> DateTime.add(-1, :day) |> DateTime.truncate(:second)
      {:ok, user} = Billing.update_subscription(user, %{current_period_ends_at: past})

      {:ok, updated} = Billing.check_subscription_expiry(user)

      assert updated.plan_id == free_plan.id
      assert updated.subscription_status == "none"
      assert updated.polar_subscription_id == nil
      # Should keep remaining seconds
      assert updated.seconds_available == 30000
    end

    test "check_subscription_expiry/1 keeps active cancelled subscription", %{user: user, pro_plan: pro_plan} do
      # Set period end in the future
      future = DateTime.utc_now() |> DateTime.add(10, :day) |> DateTime.truncate(:second)
      {:ok, user} = Billing.update_subscription(user, %{current_period_ends_at: future})

      {:ok, updated} = Billing.check_subscription_expiry(user)

      assert updated.plan_id == pro_plan.id
      assert updated.subscription_status == "cancelled"
    end

    test "check_subscription_expiry/1 ignores active subscriptions", %{user: user, pro_plan: pro_plan} do
      {:ok, user} = Billing.update_subscription(user, %{subscription_status: "active"})

      {:ok, updated} = Billing.check_subscription_expiry(user)

      assert updated.plan_id == pro_plan.id
      assert updated.subscription_status == "active"
    end

    test "subscription_expired?/1 returns true for past date", %{user: user} do
      past = DateTime.utc_now() |> DateTime.add(-1, :hour) |> DateTime.truncate(:second)
      {:ok, user} = Billing.update_subscription(user, %{current_period_ends_at: past})

      assert Billing.subscription_expired?(user)
    end

    test "subscription_expired?/1 returns false for future date", %{user: user} do
      future = DateTime.utc_now() |> DateTime.add(1, :hour) |> DateTime.truncate(:second)
      {:ok, user} = Billing.update_subscription(user, %{current_period_ends_at: future})

      refute Billing.subscription_expired?(user)
    end

    test "subscription_expired?/1 returns false for nil date", %{user: user} do
      {:ok, user} = Billing.update_subscription(user, %{current_period_ends_at: nil})

      refute Billing.subscription_expired?(user)
    end

    test "check_subscription_expiry/1 ignores past_due subscriptions", %{user: user, pro_plan: pro_plan} do
      # past_due users keep access during grace period
      {:ok, user} = Billing.update_subscription(user, %{subscription_status: "past_due"})

      {:ok, updated} = Billing.check_subscription_expiry(user)

      # Should remain unchanged
      assert updated.plan_id == pro_plan.id
      assert updated.subscription_status == "past_due"
    end
  end

  describe "webhook idempotency" do
    test "webhook_processed?/1 returns false for new event" do
      refute Billing.webhook_processed?("evt_new_123")
    end

    test "mark_webhook_processed/2 records the event" do
      assert {:ok, webhook} = Billing.mark_webhook_processed("evt_123", "subscription.active")
      assert webhook.event_id == "evt_123"
      assert webhook.event_type == "subscription.active"
    end

    test "webhook_processed?/1 returns true after marking" do
      Billing.mark_webhook_processed("evt_456", "subscription.active")
      assert Billing.webhook_processed?("evt_456")
    end

    test "mark_webhook_processed/2 returns error for duplicate" do
      assert {:ok, _} = Billing.mark_webhook_processed("evt_dup", "subscription.active")
      assert {:error, _} = Billing.mark_webhook_processed("evt_dup", "subscription.active")
    end
  end
end
