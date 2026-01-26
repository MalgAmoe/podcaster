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
  alias PoddyclipBackend.Accounts.User
  alias PoddyclipBackend.Billing.{Plan, ProcessedWebhook}
  require Logger

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
end
