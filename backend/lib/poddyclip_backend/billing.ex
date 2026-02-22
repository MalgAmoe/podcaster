defmodule PoddyclipBackend.Billing do
  @moduledoc """
  The Billing context handles subscription management and seconds tracking.

  ## Plans

  - **free**: 900 seconds (15 minutes), no card required
  - **pro**: $15/mo, 54000 seconds (15 hours)

  ## Seconds Management

  Users start with seconds based on their plan. Seconds are deducted when
  processing jobs and refunded if jobs fail. When a subscription renews,
  seconds are reset to the plan amount.
  """

  import Ecto.Query, warn: false
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.{User, UserNotifier}
  alias PoddyclipBackend.Billing.{MinutePack, Plan, ProcessedWebhook, Promo}
  require Logger

  # ----- Promos -----

  @doc """
  Finds the first active promo (within time window, under claim limit).
  Returns the promo or nil.
  """
  def find_active_promo do
    now = DateTime.utc_now()

    from(p in Promo,
      where: p.active == true,
      where: p.starts_at <= ^now,
      where: p.expires_at > ^now,
      where: p.claims_count < p.max_claims,
      order_by: [asc: p.starts_at],
      limit: 1
    )
    |> Repo.one()
  end

  @doc """
  Atomically increments claims_count for a promo, but only if still under limit.
  Returns `{:ok, promo}` if claimed, `{:error, :exhausted}` if no slots left.
  """
  def claim_promo(%Promo{id: id}) do
    {count, _} =
      from(p in Promo,
        where: p.id == ^id,
        where: p.claims_count < p.max_claims
      )
      |> Repo.update_all(inc: [claims_count: 1])

    if count == 1 do
      {:ok, Repo.get!(Promo, id)}
    else
      {:error, :exhausted}
    end
  end

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
          seconds: 900,
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

  # ----- Minute Packs -----

  @doc """
  Gets all valid (non-expired, with remaining seconds) minute packs for a user.
  Returns packs ordered by expiry date (FIFO - earliest expiring first).
  """
  def get_valid_packs(user_id) do
    now = DateTime.utc_now()

    MinutePack
    |> where([p], p.user_id == ^user_id)
    |> where([p], p.expires_at > ^now)
    |> where([p], p.seconds_remaining > 0)
    |> order_by([p], asc: p.expires_at)
    |> Repo.all()
  end

  @doc """
  Gets a summary of the user's minute packs.

  Returns `%{total_seconds: integer, pack_count: integer, next_expiry: DateTime | nil}`
  """
  def get_pack_summary(user_id) do
    packs = get_valid_packs(user_id)

    %{
      total_seconds: Enum.reduce(packs, 0, &(&1.seconds_remaining + &2)),
      pack_count: length(packs),
      next_expiry: List.first(packs) && List.first(packs).expires_at
    }
  end

  @doc """
  Gets the total seconds available for a user (subscription + valid packs).
  """
  def get_total_seconds_available(%User{seconds_available: subscription_seconds} = user) do
    pack_summary = get_pack_summary(user.id)
    subscription_seconds + pack_summary.total_seconds
  end

  @doc """
  Creates a minute pack for a user (called from Polar webhook).

  Returns `{:ok, minute_pack}` or `{:error, changeset}`.
  The polar_order_id provides idempotency for webhook retries.
  """
  def create_minute_pack(user_id, polar_order_id \\ nil) do
    %MinutePack{}
    |> MinutePack.create_changeset(%{user_id: user_id, polar_order_id: polar_order_id})
    |> Repo.insert()
    |> case do
      {:ok, pack} ->
        Logger.info("Minute pack created",
          user_id: user_id,
          pack_id: pack.id,
          polar_order_id: polar_order_id,
          seconds: pack.seconds_total,
          expires_at: pack.expires_at
        )

        # Broadcast update to LiveView
        case Accounts.get_user(user_id) do
          nil -> :ok
          user -> broadcast_user_update(Repo.preload(user, :plan))
        end

        {:ok, pack}

      {:error, %Ecto.Changeset{errors: [polar_order_id: _]}} = error ->
        # Duplicate order ID - idempotent, return success
        Logger.info("Duplicate minute pack order, ignoring",
          user_id: user_id,
          polar_order_id: polar_order_id
        )
        # Return the existing pack
        case Repo.get_by(MinutePack, polar_order_id: polar_order_id) do
          nil -> error
          pack -> {:ok, pack}
        end

      error ->
        error
    end
  end

  @doc """
  Deletes a minute pack by its Polar order ID (called on refund webhook).

  Returns `{:ok, minute_pack}` if found and deleted, `{:error, :not_found}` if not found.
  """
  def delete_minute_pack_by_order_id(polar_order_id) when is_binary(polar_order_id) do
    case Repo.get_by(MinutePack, polar_order_id: polar_order_id) do
      nil ->
        {:error, :not_found}

      pack ->
        case Repo.delete(pack) do
          {:ok, deleted_pack} ->
            Logger.info("Minute pack deleted due to refund",
              pack_id: deleted_pack.id,
              user_id: deleted_pack.user_id,
              polar_order_id: polar_order_id,
              seconds_remaining: deleted_pack.seconds_remaining
            )
            {:ok, deleted_pack}

          error ->
            error
        end
    end
  end

  # ----- Seconds -----

  @doc """
  Checks if a user has enough seconds for a job.
  Includes both subscription seconds and minute pack seconds.

  ## Examples

      iex> has_seconds?(user, 300)
      true

      iex> has_seconds?(user, 100000)
      false
  """
  def has_seconds?(%User{} = user, required) do
    get_total_seconds_available(user) >= required
  end

  @doc """
  Deducts seconds from a user's balance.

  Order of deduction:
  1. Subscription seconds first
  2. Minute packs (FIFO by expiry date)

  Returns `{:ok, user}` if successful, `{:error, :insufficient_seconds}` if
  the user doesn't have enough seconds (including packs).

  ## Examples

      iex> deduct_seconds(user, 300)
      {:ok, %User{seconds_available: 600}}

      iex> deduct_seconds(user, 100000)
      {:error, :insufficient_seconds}
  """
  def deduct_seconds(%User{seconds_available: subscription_seconds} = user, amount) when amount > 0 do
    total_available = get_total_seconds_available(user)

    if total_available >= amount do
      # First, deduct from subscription seconds
      {subscription_deduct, remaining_to_deduct} =
        if subscription_seconds >= amount do
          {amount, 0}
        else
          {subscription_seconds, amount - subscription_seconds}
        end

      # Update subscription seconds
      new_subscription_seconds = subscription_seconds - subscription_deduct

      result =
        Repo.transaction(fn ->
          # Update user's subscription seconds
          {:ok, updated_user} =
            user
            |> Ecto.Changeset.change(seconds_available: new_subscription_seconds)
            |> Repo.update()

          # Deduct remaining from packs (FIFO)
          if remaining_to_deduct > 0 do
            deduct_from_packs(user.id, remaining_to_deduct)
          end

          updated_user
        end)

      case result do
        {:ok, updated_user} ->
          Logger.info("Seconds deducted",
            user_id: user.id,
            amount: amount,
            subscription_deducted: subscription_deduct,
            packs_deducted: remaining_to_deduct,
            subscription_remaining: updated_user.seconds_available
          )

          # Check if crossing 80% threshold and send notification
          maybe_send_low_seconds_notification(updated_user, subscription_seconds, amount)

          {:ok, updated_user}

        {:error, reason} ->
          Logger.error("Failed to deduct seconds",
            user_id: user.id,
            amount: amount,
            error: inspect(reason)
          )
          {:error, reason}
      end
    else
      Logger.warning("Insufficient seconds for deduction",
        user_id: user.id,
        requested: amount,
        subscription_available: subscription_seconds,
        total_available: total_available
      )
      {:error, :insufficient_seconds}
    end
  end

  # Deducts seconds from packs in FIFO order (by expiry date)
  defp deduct_from_packs(user_id, amount) do
    packs = get_valid_packs(user_id)
    deduct_from_packs_recursive(packs, amount)
  end

  defp deduct_from_packs_recursive([], _remaining), do: :ok
  defp deduct_from_packs_recursive(_packs, 0), do: :ok

  defp deduct_from_packs_recursive([pack | rest], remaining) do
    deduct_from_pack = min(pack.seconds_remaining, remaining)
    new_remaining = remaining - deduct_from_pack

    pack
    |> MinutePack.deduct_changeset(deduct_from_pack)
    |> Repo.update!()

    Logger.info("Deducted from minute pack",
      pack_id: pack.id,
      deducted: deduct_from_pack,
      pack_remaining: pack.seconds_remaining - deduct_from_pack
    )

    deduct_from_packs_recursive(rest, new_remaining)
  end

  @doc """
  Refunds seconds to a user's balance.

  Used when a job fails and the estimated seconds should be returned.

  ## Examples

      iex> refund_seconds(user, 300)
      {:ok, %User{seconds_available: 1200}}
  """
  def refund_seconds(%User{seconds_available: available} = user, amount) when amount > 0 do
    case user
         |> Ecto.Changeset.change(seconds_available: available + amount)
         |> Repo.update() do
      {:ok, updated_user} ->
        Logger.info("Seconds refunded",
          user_id: user.id,
          amount: amount,
          previous: available,
          new_balance: updated_user.seconds_available
        )
        {:ok, updated_user}

      error ->
        error
    end
  end

  @doc """
  Adjusts seconds after job completion.

  If actual usage differs from estimated, adjusts the user's balance.
  Positive adjustment = refund (job used less), negative = deduct more.

  ## Examples

      iex> adjust_seconds(user, 120)  # Job used 120 fewer seconds
      {:ok, %User{}}
  """
  def adjust_seconds(%User{seconds_available: available} = user, adjustment) do
    new_balance = max(0, available + adjustment)

    user
    |> Ecto.Changeset.change(seconds_available: new_balance)
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
        current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)
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
  Resets a user's seconds to their plan amount.

  Called when a subscription renews.
  """
  def reset_subscription_seconds(%User{} = user) do
    user = Repo.preload(user, :plan)

    if user.plan do
      user
      |> Ecto.Changeset.change(seconds_available: user.plan.seconds)
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
  - `:seconds_available` - Available seconds (set when upgrading)
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
    munch_plan = get_plan_by_name("munch")
    period_end = parse_polar_datetime(sub["current_period_end"])

    attrs = %{
      subscription_status: "active",
      polar_subscription_id: sub["id"],
      current_period_ends_at: period_end,
      plan_id: munch_plan && munch_plan.id
    }

    # If upgrading from free/none, also set seconds
    attrs =
      if user.subscription_status != "active" && munch_plan do
        Map.put(attrs, :seconds_available, munch_plan.seconds)
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
        current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second),
        plan_id: free_plan.id,
        seconds_available: free_plan.seconds
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

  # ----- Low Seconds Notifications -----

  @low_seconds_threshold 0.80

  defp maybe_send_low_seconds_notification(user, previous_seconds, _deducted_amount) do
    # Get user's plan to calculate percentage
    user = Repo.preload(user, :plan)
    plan_seconds = (user.plan && user.plan.seconds) || 900

    # Calculate usage percentages before and after deduction
    previous_used_pct = (plan_seconds - previous_seconds) / plan_seconds
    current_used_pct = (plan_seconds - user.seconds_available) / plan_seconds

    # Check if we just crossed the 80% threshold
    if previous_used_pct < @low_seconds_threshold and current_used_pct >= @low_seconds_threshold do
      send_low_seconds_notification(user, plan_seconds)
    end
  end

  defp send_low_seconds_notification(user, plan_seconds) do
    # Check user preferences and spam prevention
    if User.notification_enabled?(user, :low_time) and
       Accounts.should_send_low_time_notification?(user) do
      percent_used = round((plan_seconds - user.seconds_available) / plan_seconds * 100)
      # Convert seconds to minutes for user-friendly notification
      minutes_remaining = div(user.seconds_available, 60)

      try do
        UserNotifier.deliver_low_time(user, minutes_remaining, percent_used)
        Accounts.record_low_time_notification(user)

        Logger.info("Low seconds notification sent",
          user_id: user.id,
          seconds_remaining: user.seconds_available,
          percent_used: percent_used
        )
      rescue
        e ->
          Logger.error("Failed to send low seconds notification",
            user_id: user.id,
            error: Exception.message(e)
          )
      end
    end
  end
end
