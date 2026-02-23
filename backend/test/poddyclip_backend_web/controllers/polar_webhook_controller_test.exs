defmodule PoddyclipBackendWeb.PolarWebhookControllerTest do
  use PoddyclipBackendWeb.ConnCase

  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Repo

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  # Test secret - used directly as HMAC key (production code uses secret string as-is)
  @test_secret "test_webhook_secret_for_testing_12345"

  setup do
    # Configure test secret
    Application.put_env(:poddyclip_backend, :polar_webhook_secret, @test_secret)

    on_exit(fn ->
      Application.delete_env(:poddyclip_backend, :polar_webhook_secret)
    end)

    free_plan = free_plan_fixture()
    munch_plan = munch_plan_fixture()

    %{free_plan: free_plan, munch_plan: munch_plan}
  end

  defp sign_webhook(body, webhook_id \\ "wh_test_#{System.unique_integer([:positive])}") do
    timestamp = to_string(System.system_time(:second))
    signed_payload = "#{webhook_id}.#{timestamp}.#{body}"
    signature = :crypto.mac(:hmac, :sha256, @test_secret, signed_payload) |> Base.encode64()

    {webhook_id, timestamp, "v1,#{signature}"}
  end

  defp post_webhook(conn, body, opts \\ []) when is_binary(body) do
    # Decode and re-encode to get canonical JSON that matches what controller will produce
    params = Jason.decode!(body)
    canonical_body = Jason.encode!(params)

    {webhook_id, timestamp, signature} =
      case Keyword.get(opts, :webhook_id) do
        nil -> sign_webhook(canonical_body)
        id -> sign_webhook(canonical_body, id)
      end

    conn
    |> put_req_header("content-type", "application/json")
    |> put_req_header("webhook-id", webhook_id)
    |> put_req_header("webhook-timestamp", timestamp)
    |> put_req_header("webhook-signature", signature)
    |> post("/api/webhooks/polar", params)
  end

  describe "handle/2" do
    test "returns 401 for invalid signature", %{conn: conn} do
      params = %{"type" => "test", "data" => %{}}

      conn =
        conn
        |> put_req_header("content-type", "application/json")
        |> put_req_header("webhook-id", "wh_123")
        |> put_req_header("webhook-timestamp", to_string(System.system_time(:second)))
        |> put_req_header("webhook-signature", "v1,invalid")
        |> post("/api/webhooks/polar", params)

      assert json_response(conn, 401)["error"] == "Invalid signature"
    end

    test "returns error when secret not configured", %{conn: conn} do
      Application.delete_env(:poddyclip_backend, :polar_webhook_secret)
      body = ~s({"type": "test", "data": {}})

      conn = post_webhook(conn, body)

      assert json_response(conn, 500)["error"] =~ "not configured"
    end

    test "handles duplicate events gracefully", %{conn: conn} do
      body = ~s({"id": "evt_dup_test", "type": "subscription.created", "data": {}})
      fixed_id = "wh_dup_test"

      # First request - event gets processed
      conn1 = post_webhook(conn, body, webhook_id: fixed_id)
      assert json_response(conn1, 200)["status"] == "ok"

      # Second request (duplicate) - same webhook-id header
      conn2 = post_webhook(build_conn(), body, webhook_id: fixed_id)
      assert json_response(conn2, 200)["status"] == "already_processed"
    end
  end

  describe "subscription.active" do
    test "activates pro subscription for existing user", %{conn: conn, munch_plan: munch_plan} do
      # Create user and link to Polar customer_id (simulates checkout flow)
      customer_id = "cus_test_#{System.unique_integer([:positive])}"
      user = user_fixture()

      # Set polar_customer_id to simulate user who completed checkout
      {:ok, user} =
        user
        |> Ecto.Changeset.change(%{polar_customer_id: customer_id})
        |> Repo.update()

      body =
        Jason.encode!(%{
          id: "evt_active_#{System.unique_integer()}",
          type: "subscription.active",
          data: %{
            subscription: %{
              id: "sub_test_123",
              customer_id: customer_id,
              customer: %{email: user.email},
              product: %{id: munch_plan.polar_product_id},
              current_period_end: System.system_time(:second) + 86400 * 30
            }
          }
        })

      conn = post_webhook(conn, body)
      assert json_response(conn, 200)["status"] == "ok"

      # Verify user was updated
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.plan_id == munch_plan.id
      assert updated_user.seconds_available == 54000
      assert updated_user.subscription_status == "active"
      assert updated_user.polar_subscription_id == "sub_test_123"
      assert updated_user.polar_customer_id == customer_id
    end

    test "handles user not found", %{conn: conn, munch_plan: munch_plan} do
      body =
        Jason.encode!(%{
          id: "evt_nouser_#{System.unique_integer()}",
          type: "subscription.active",
          data: %{
            subscription: %{
              id: "sub_nouser",
              customer: %{email: "nonexistent@example.com"},
              product: %{id: munch_plan.polar_product_id}
            }
          }
        })

      conn = post_webhook(conn, body)
      response = json_response(conn, 200)
      assert response["status"] == "ok"
      assert response["warning"] == "user_not_found"
    end
  end

  describe "subscription.updated" do
    test "resets minutes on renewal", %{conn: conn, munch_plan: munch_plan} do
      user = user_fixture()

      # Set up user as pro subscriber with used minutes
      old_period_end = DateTime.utc_now() |> DateTime.add(-1, :day) |> DateTime.truncate(:second)

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 6000,
          subscription_status: "active",
          polar_subscription_id: "sub_renew_123",
          current_period_ends_at: old_period_end
        })

      # New period (renewal)
      new_period_end = DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)

      body =
        Jason.encode!(%{
          id: "evt_renew_#{System.unique_integer()}",
          type: "subscription.updated",
          data: %{
            subscription: %{
              id: "sub_renew_123",
              current_period_end: DateTime.to_unix(new_period_end)
            }
          }
        })

      conn = post_webhook(conn, body)
      response = json_response(conn, 200)
      assert response["status"] == "ok"
      assert response["renewed"] == true

      # Verify minutes were reset
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 54000
    end

    test "doesn't reset minutes for non-renewal update", %{conn: conn, munch_plan: munch_plan} do
      user = user_fixture()

      # Set up user as pro subscriber
      period_end = DateTime.utc_now() |> DateTime.add(15, :day) |> DateTime.truncate(:second)

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 30000,
          subscription_status: "active",
          polar_subscription_id: "sub_update_456",
          current_period_ends_at: period_end
        })

      # Update with same period end (not a renewal)
      body =
        Jason.encode!(%{
          id: "evt_update_#{System.unique_integer()}",
          type: "subscription.updated",
          data: %{
            subscription: %{
              id: "sub_update_456",
              current_period_end: DateTime.to_unix(period_end)
            }
          }
        })

      conn = post_webhook(conn, body)
      response = json_response(conn, 200)
      assert response["renewed"] == false

      # Minutes should not be reset
      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.seconds_available == 30000
    end
  end

  describe "subscription.canceled" do
    test "marks subscription as cancelled", %{conn: conn, munch_plan: munch_plan} do
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 30000,
          subscription_status: "active",
          polar_subscription_id: "sub_cancel_123"
        })

      body =
        Jason.encode!(%{
          id: "evt_cancel_#{System.unique_integer()}",
          type: "subscription.canceled",
          data: %{
            subscription: %{id: "sub_cancel_123"}
          }
        })

      conn = post_webhook(conn, body)
      assert json_response(conn, 200)["status"] == "ok"

      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.subscription_status == "cancelled"
      # Should keep access until period ends
      assert updated_user.plan_id == munch_plan.id
      assert updated_user.seconds_available == 30000
    end
  end

  describe "subscription.uncanceled" do
    test "reactivates cancelled subscription", %{conn: conn, munch_plan: munch_plan} do
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 30000,
          subscription_status: "cancelled",
          polar_subscription_id: "sub_uncancel_123",
          current_period_ends_at: DateTime.utc_now() |> DateTime.add(10, :day) |> DateTime.truncate(:second)
        })

      body =
        Jason.encode!(%{
          id: "evt_uncancel_#{System.unique_integer()}",
          type: "subscription.uncanceled",
          data: %{
            subscription: %{id: "sub_uncancel_123"}
          }
        })

      conn = post_webhook(conn, body)
      assert json_response(conn, 200)["status"] == "ok"

      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      # Should be reactivated
      assert updated_user.subscription_status == "active"
      # Should keep pro plan and remaining minutes
      assert updated_user.plan_id == munch_plan.id
      assert updated_user.seconds_available == 30000
    end
  end

  describe "subscription.past_due" do
    test "marks subscription as past_due", %{conn: conn, munch_plan: munch_plan} do
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 30000,
          subscription_status: "active",
          polar_subscription_id: "sub_pastdue_123"
        })

      body =
        Jason.encode!(%{
          id: "evt_pastdue_#{System.unique_integer()}",
          type: "subscription.past_due",
          data: %{
            subscription: %{id: "sub_pastdue_123"}
          }
        })

      conn = post_webhook(conn, body)
      assert json_response(conn, 200)["status"] == "ok"

      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      # Should be marked as past_due
      assert updated_user.subscription_status == "past_due"
      # Should keep access while in grace period
      assert updated_user.plan_id == munch_plan.id
      assert updated_user.seconds_available == 30000
    end
  end

  describe "subscription.revoked" do
    test "removes access immediately", %{conn: conn, free_plan: free_plan, munch_plan: munch_plan} do
      user = user_fixture()

      {:ok, user} =
        Billing.update_subscription(user, %{
          plan_id: munch_plan.id,
          seconds_available: 30000,
          subscription_status: "active",
          polar_subscription_id: "sub_revoke_123",
          current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)
        })

      body =
        Jason.encode!(%{
          id: "evt_revoke_#{System.unique_integer()}",
          type: "subscription.revoked",
          data: %{
            subscription: %{id: "sub_revoke_123"}
          }
        })

      conn = post_webhook(conn, body)
      assert json_response(conn, 200)["status"] == "ok"

      updated_user = Repo.get!(PoddyclipBackend.Accounts.User, user.id)
      assert updated_user.subscription_status == "none"
      assert updated_user.plan_id == free_plan.id
      # Should keep remaining minutes
      assert updated_user.seconds_available == 30000
      assert updated_user.polar_subscription_id == nil
      # Should start a new 30-day free period (not nil)
      assert updated_user.current_period_ends_at != nil
      assert DateTime.compare(updated_user.current_period_ends_at, DateTime.utc_now()) == :gt
    end
  end

  describe "unhandled events" do
    test "ignores unknown event types", %{conn: conn} do
      body =
        Jason.encode!(%{
          id: "evt_unknown_#{System.unique_integer()}",
          type: "some.unknown.event",
          data: %{}
        })

      conn = post_webhook(conn, body)
      response = json_response(conn, 200)
      assert response["status"] == "ok"
      assert response["ignored"] == true
    end
  end
end
