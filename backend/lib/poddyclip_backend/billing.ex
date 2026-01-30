defmodule PoddyclipBackend.Billing do
  @moduledoc """
  The Billing context handles subscription management and minute tracking.

  ## Plans

  - **free**: 15 minutes, no card required
  - **pro**: $15/mo, 900 minutes (15 hours)

  ## Minute Management

  Users start with minutes based on their plan. Minutes are deducted when
  processing jobs and refunded if jobs fail. When a subscription renews,
  minutes are reset to the plan amount.
  """

  import Ecto.Query, warn: false
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.{User, UserNotifier}
  alias PoddyclipBackend.Billing.{Plan, ProcessedWebhook}
  require Logger

  # ----- PubSub -----

  @doc """
  Subscribe to updates for a user's billing/subscription changes.
  Updates are broadcast as {:user_updated, user} messages.
  """
  def subscribe(user_id) do
    Phoenix.PubSub.subscribe(PoddyclipBackend.PubSub, "user:#{user_id}")
  end

  defp broadcast_user_update(user) do
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "user:#{user.id}", {:user_updated, user})
  end

  # ----- Plans -----

  @doc """
  Gets a plan by name.

  ## Examples

      iex> get_plan_by_name("free")
      %Plan{}

      iex> get_plan_by_name("nonexistent")
      nil
  """
  def get_plan_by_name(name) when is_binary(name) do
    Repo.get_by(Plan, name: name)
  end

  @doc """
  Gets a plan by ID.
  """
  def get_plan(id) do
    Repo.get(Plan, id)
  end

  @doc """
  Gets the free plan, creating it if it doesn't exist.
  """
  def get_or_create_free_plan do
    case get_plan_by_name("free") do
      nil ->
        %Plan{}
        |> Plan.changeset(%{
          name: "free",
          display_name: "Free",
          minutes: 15,
          price_cents: 0
        })
        |> Repo.insert!()

      plan ->
        plan
    end
  end

  @doc """
  Gets a user's current plan.

  Returns the plan if the user has one, otherwise returns nil.
  """
  def get_user_plan(%User{} = user) do
    user = Repo.preload(user, :plan)
    user.plan
  end

  @doc """
  Lists all active plans.
  """
  def list_active_plans do
    Plan
    |> where([p], p.active == true)
    |> order_by([p], asc: p.price_cents)
    |> Repo.all()
  end

  # ----- Minutes -----

  @doc """
  Checks if a user has enough minutes for a job.

  ## Examples

      iex> has_minutes?(user, 5)
      true

      iex> has_minutes?(user, 1000)
      false
  """
  def has_minutes?(%User{minutes_available: available}, required) do
    available >= required
  end

  @doc """
  Deducts minutes from a user's balance.

  Returns `{:ok, user}` if successful, `{:error, :insufficient_minutes}` if
  the user doesn't have enough minutes.

  ## Examples

      iex> deduct_minutes(user, 5)
      {:ok, %User{minutes_available: 10}}

      iex> deduct_minutes(user, 1000)
      {:error, :insufficient_minutes}
  """
  def deduct_minutes(%User{minutes_available: available} = user, amount) when amount > 0 do
    if available >= amount do
      case user
           |> Ecto.Changeset.change(minutes_available: available - amount)
           |> Repo.update() do
        {:ok, updated_user} ->
          Logger.info("Minutes deducted",
            user_id: user.id,
            amount: amount,
            previous: available,
            remaining: updated_user.minutes_available
          )

          # Check if crossing 80% threshold and send notification
          maybe_send_low_minutes_notification(updated_user, available, amount)

          {:ok, updated_user}

        error ->
          error
      end
    else
      Logger.warning("Insufficient minutes for deduction",
        user_id: user.id,
        requested: amount,
        available: available
      )
      {:error, :insufficient_minutes}
    end
  end

  @doc """
  Refunds minutes to a user's balance.

  Used when a job fails and the estimated minutes should be returned.

  ## Examples

      iex> refund_minutes(user, 5)
      {:ok, %User{minutes_available: 20}}
  """
  def refund_minutes(%User{minutes_available: available} = user, amount) when amount > 0 do
    case user
         |> Ecto.Changeset.change(minutes_available: available + amount)
         |> Repo.update() do
      {:ok, updated_user} ->
        Logger.info("Minutes refunded",
          user_id: user.id,
          amount: amount,
          previous: available,
          new_balance: updated_user.minutes_available
        )
        {:ok, updated_user}

      error ->
        error
    end
  end

  @doc """
  Adjusts minutes after job completion.

  If actual usage differs from estimated, adjusts the user's balance.
  Positive adjustment = refund (job used less), negative = deduct more.

  ## Examples

      iex> adjust_minutes(user, 2)  # Job used 2 fewer minutes
      {:ok, %User{}}
  """
  def adjust_minutes(%User{minutes_available: available} = user, adjustment) do
    new_balance = max(0, available + adjustment)

    user
    |> Ecto.Changeset.change(minutes_available: new_balance)
    |> Repo.update()
  end

  # ----- Subscriptions -----

  @doc """
  Checks if a cancelled subscription has expired and downgrades if needed.

  Returns `{:ok, user}` with the possibly updated user. If the subscription
  period has ended for a cancelled user, they are downgraded to the free plan.
  """
  def check_subscription_expiry(%User{subscription_status: "cancelled"} = user) do
    if subscription_expired?(user) do
      free_plan = get_or_create_free_plan()

      update_subscription(user, %{
        plan_id: free_plan.id,
        subscription_status: "none",
        polar_subscription_id: nil,
        current_period_ends_at: nil
        # Keep remaining minutes
      })
    else
      {:ok, user}
    end
  end

  def check_subscription_expiry(%User{} = user), do: {:ok, user}

  @doc """
  Checks if the user's subscription period has expired.
  """
  def subscription_expired?(%User{current_period_ends_at: nil}), do: false
  def subscription_expired?(%User{current_period_ends_at: period_end}) do
    DateTime.compare(DateTime.utc_now(), period_end) == :gt
  end

  @doc """
  Resets a user's minutes to their plan amount.

  Called when a subscription renews.
  """
  def reset_subscription_minutes(%User{} = user) do
    user = Repo.preload(user, :plan)

    if user.plan do
      user
      |> Ecto.Changeset.change(minutes_available: user.plan.minutes)
      |> Repo.update()
    else
      {:error, :no_plan}
    end
  end

  @doc """
  Updates a user's subscription status and related fields.

  ## Fields

  - `:subscription_status` - "none", "active", "cancelled"
  - `:polar_customer_id` - Polar customer ID
  - `:polar_subscription_id` - Polar subscription ID
  - `:current_period_ends_at` - When the current billing period ends
  - `:plan_id` - The plan ID
  - `:minutes_available` - Available minutes (set when upgrading)
  """
  def update_subscription(%User{} = user, attrs) do
    case user
         |> Ecto.Changeset.change(attrs)
         |> Repo.update() do
      {:ok, updated_user} ->
        # Log subscription status changes
        if Map.has_key?(attrs, :subscription_status) do
          Logger.info("Subscription updated",
            user_id: user.id,
            prev_status: user.subscription_status,
            new_status: attrs[:subscription_status],
            plan_id: attrs[:plan_id] || user.plan_id
          )
        end

        # Broadcast update for LiveView subscribers
        broadcast_user_update(Repo.preload(updated_user, :plan))

        {:ok, updated_user}

      error ->
        error
    end
  end

  @doc """
  Finds a user by their Polar subscription ID.
  """
  def get_user_by_subscription_id(subscription_id) when is_binary(subscription_id) do
    Repo.get_by(User, polar_subscription_id: subscription_id)
  end

  @doc """
  Finds a user by their Polar customer ID.
  """
  def get_user_by_customer_id(customer_id) when is_binary(customer_id) do
    Repo.get_by(User, polar_customer_id: customer_id)
  end

  # ----- Webhook Idempotency -----

  @doc """
  Checks if a webhook event has already been processed.

  ## Examples

      iex> webhook_processed?("evt_123")
      false
  """
  def webhook_processed?(event_id) when is_binary(event_id) do
    ProcessedWebhook
    |> where([w], w.event_id == ^event_id)
    |> Repo.exists?()
  end

  @doc """
  Marks a webhook event as processed.

  Returns `{:ok, webhook}` if successful, `{:error, changeset}` if the event
  was already processed (unique constraint violation).
  """
  def mark_webhook_processed(event_id, event_type) do
    %ProcessedWebhook{}
    |> ProcessedWebhook.changeset(%{event_id: event_id, event_type: event_type})
    |> Repo.insert()
  end

  # ----- Subscription Sync -----

  @doc """
  Syncs a user's subscription status from Polar API.

  Fetches the current subscription state from Polar and updates the user's
  local subscription status, plan, and period dates.

  Returns `{:ok, user}` with the updated user, or `{:error, reason}` on failure.
  """
  def sync_subscription_from_polar(%User{polar_customer_id: nil} = user) do
    # No Polar customer ID, nothing to sync
    {:ok, user}
  end

  def sync_subscription_from_polar(%User{polar_customer_id: customer_id} = user) do
    alias PoddyclipBackend.Polar

    case Polar.get_customer_subscriptions(customer_id) do
      {:ok, subscriptions} ->
        # Find active subscription (if any)
        active_sub = Enum.find(subscriptions, &(&1["status"] == "active"))

        case active_sub do
          nil ->
            # No active subscription - check if cancelled or none
            cancelled_sub = Enum.find(subscriptions, &(&1["status"] == "canceled"))

            if cancelled_sub do
              # Subscription was cancelled
              sync_cancelled_subscription(user, cancelled_sub)
            else
              # No subscription at all - downgrade to free
              sync_no_subscription(user)
            end

          sub ->
            # Has active subscription - sync it
            sync_active_subscription(user, sub)
        end

      {:error, :no_access_token} ->
        Logger.warning("Cannot sync subscription: POLAR_ACCESS_TOKEN not configured",
          user_id: user.id
        )
        {:error, :no_access_token}

      {:error, reason} ->
        Logger.error("Failed to fetch subscriptions from Polar",
          user_id: user.id,
          customer_id: customer_id,
          error: inspect(reason)
        )
        {:error, reason}
    end
  end

  defp sync_active_subscription(user, sub) do
    pro_plan = get_plan_by_name("pro")
    period_end = parse_polar_datetime(sub["current_period_end"])

    attrs = %{
      subscription_status: "active",
      polar_subscription_id: sub["id"],
      current_period_ends_at: period_end,
      plan_id: pro_plan && pro_plan.id
    }

    # If upgrading from free/none, also set minutes
    attrs =
      if user.subscription_status != "active" && pro_plan do
        Map.put(attrs, :minutes_available, pro_plan.minutes)
      else
        attrs
      end

    Logger.info("Synced active subscription from Polar",
      user_id: user.id,
      subscription_id: sub["id"],
      period_end: period_end
    )

    update_subscription(user, attrs)
  end

  defp sync_cancelled_subscription(user, sub) do
    period_end = parse_polar_datetime(sub["current_period_end"]) || parse_polar_datetime(sub["ended_at"])

    attrs = %{
      subscription_status: "cancelled",
      polar_subscription_id: sub["id"],
      current_period_ends_at: period_end
    }

    Logger.info("Synced cancelled subscription from Polar",
      user_id: user.id,
      subscription_id: sub["id"],
      period_end: period_end
    )

    update_subscription(user, attrs)
  end

  defp sync_no_subscription(user) do
    free_plan = get_or_create_free_plan()

    # Only downgrade if currently has a paid status
    if user.subscription_status in ["active", "cancelled"] do
      Logger.info("No active Polar subscription, downgrading to free",
        user_id: user.id,
        prev_status: user.subscription_status
      )

      update_subscription(user, %{
        subscription_status: "none",
        polar_subscription_id: nil,
        current_period_ends_at: nil,
        plan_id: free_plan.id,
        minutes_available: free_plan.minutes
      })
    else
      {:ok, user}
    end
  end

  defp parse_polar_datetime(nil), do: nil
  defp parse_polar_datetime(datetime_str) when is_binary(datetime_str) do
    case DateTime.from_iso8601(datetime_str) do
      {:ok, dt, _offset} -> DateTime.truncate(dt, :second)
      _ -> nil
    end
  end

  # ----- Low Minutes Notifications -----

  @low_minutes_threshold 0.80

  defp maybe_send_low_minutes_notification(user, previous_minutes, _deducted_amount) do
    # Get user's plan to calculate percentage
    user = Repo.preload(user, :plan)
    plan_minutes = (user.plan && user.plan.minutes) || 15

    # Calculate usage percentages before and after deduction
    previous_used_pct = (plan_minutes - previous_minutes) / plan_minutes
    current_used_pct = (plan_minutes - user.minutes_available) / plan_minutes

    # Check if we just crossed the 80% threshold
    if previous_used_pct < @low_minutes_threshold and current_used_pct >= @low_minutes_threshold do
      send_low_minutes_notification(user, plan_minutes)
    end
  end

  defp send_low_minutes_notification(user, plan_minutes) do
    # Check user preferences and spam prevention
    if User.notification_enabled?(user, :low_minutes) and
       Accounts.should_send_low_minutes_notification?(user) do
      percent_used = round((plan_minutes - user.minutes_available) / plan_minutes * 100)

      try do
        UserNotifier.deliver_low_minutes(user, user.minutes_available, percent_used)
        Accounts.record_low_minutes_notification(user)

        Logger.info("Low minutes notification sent",
          user_id: user.id,
          minutes_remaining: user.minutes_available,
          percent_used: percent_used
        )
      rescue
        e ->
          Logger.error("Failed to send low minutes notification",
            user_id: user.id,
            error: Exception.message(e)
          )
      end
    end
  end
end
