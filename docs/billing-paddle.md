# Billing Integration - Paddle.com

## Model Overview

Subscription + top-up credits hybrid:
- Monthly subscription includes X minutes
- Users can buy additional minutes
- Free tier exists, can purchase minutes

## Decisions Made

| Decision | Choice |
|----------|--------|
| Subscription minute rollover | No - resets monthly |
| Purchased minutes expiry | Never (or 1 year, TBD) |
| Minute usage order | Subscription first, then purchased |
| Free tier purchases | Yes - can buy minutes |
| Pricing storage | Database - updatable without deploy |

## Database Schema

### Plans Table (configurable)

```elixir
# priv/repo/migrations/xxx_create_plans.exs
create table(:plans) do
  add :name, :string, null: false           # "free", "starter", "pro"
  add :display_name, :string, null: false   # "Free", "Starter", "Pro"
  add :paddle_product_id, :string           # null for free tier
  add :paddle_price_id, :string             # null for free tier
  add :price_cents, :integer, default: 0    # for display, Paddle is source of truth
  add :included_minutes, :integer, default: 0
  add :max_file_duration_minutes, :integer, default: 5
  add :features, :map, default: %{}         # extensible feature flags
  add :active, :boolean, default: true
  add :sort_order, :integer, default: 0

  timestamps()
end

create unique_index(:plans, [:name])
create index(:plans, [:paddle_price_id])
```

### Minute Packs Table (configurable)

```elixir
# priv/repo/migrations/xxx_create_minute_packs.exs
create table(:minute_packs) do
  add :name, :string, null: false           # "30min", "60min", "120min"
  add :display_name, :string, null: false   # "30 Minutes"
  add :paddle_product_id, :string, null: false
  add :paddle_price_id, :string, null: false
  add :price_cents, :integer, null: false   # for display
  add :minutes, :integer, null: false
  add :active, :boolean, default: true
  add :sort_order, :integer, default: 0

  timestamps()
end

create index(:minute_packs, [:paddle_price_id])
```

### Processed Webhooks Table (idempotency)

```elixir
# priv/repo/migrations/xxx_create_processed_webhooks.exs
create table(:processed_webhooks) do
  add :event_id, :string, null: false
  add :resource_key, :string, null: false, default: "default"  # e.g., "subscription", price_id
  add :event_type, :string, null: false

  timestamps(updated_at: false)
end

# Composite unique - same event can have multiple resources (multi-item transactions)
create unique_index(:processed_webhooks, [:event_id, :resource_key])
```

### User Fields (add to existing users table)

```elixir
# priv/repo/migrations/xxx_add_billing_to_users.exs
alter table(:users) do
  # Paddle identifiers
  add :paddle_customer_id, :string
  add :paddle_subscription_id, :string

  # Current plan
  add :plan_id, references(:plans), null: false, default: 1  # free plan
  add :subscription_status, :string, default: "none"  # none, active, past_due, canceled

  # Billing period (always from Paddle, never calculated)
  add :current_period_starts_at, :utc_datetime
  add :current_period_ends_at, :utc_datetime
  add :scheduled_cancel_at, :utc_datetime  # user requested cancellation, still active until period end

  # Usage tracking
  add :subscription_minutes_used, :integer, default: 0  # resets each period
  add :purchased_minutes, :integer, default: 0          # never resets
end

create index(:users, [:paddle_customer_id])
create index(:users, [:paddle_subscription_id])

# Prevent negative minutes at DB level (catches bugs)
create constraint(:users, :minutes_non_negative,
  check: "subscription_minutes_used >= 0 AND purchased_minutes >= 0"
)
```

## Seed Data

```elixir
# priv/repo/seeds.exs (or migration)

# Plans - update paddle IDs after creating products in Paddle dashboard
plans = [
  %{
    name: "free",
    display_name: "Free",
    paddle_product_id: nil,
    paddle_price_id: nil,
    price_cents: 0,
    included_minutes: 5,
    max_file_duration_minutes: 5,
    sort_order: 0
  },
  %{
    name: "starter",
    display_name: "Starter",
    paddle_product_id: "pro_xxx",  # fill after Paddle setup
    paddle_price_id: "pri_xxx",
    price_cents: 900,  # $9
    included_minutes: 60,
    max_file_duration_minutes: 30,
    sort_order: 1
  },
  %{
    name: "pro",
    display_name: "Pro",
    paddle_product_id: "pro_yyy",
    paddle_price_id: "pri_yyy",
    price_cents: 1900,  # $19
    included_minutes: 180,
    max_file_duration_minutes: 120,
    sort_order: 2
  }
]

# Minute packs
minute_packs = [
  %{
    name: "30min",
    display_name: "30 Minutes",
    paddle_product_id: "pro_aaa",
    paddle_price_id: "pri_aaa",
    price_cents: 500,  # $5
    minutes: 30,
    sort_order: 0
  },
  %{
    name: "60min",
    display_name: "60 Minutes",
    paddle_product_id: "pro_bbb",
    paddle_price_id: "pri_bbb",
    price_cents: 900,  # $9
    minutes: 60,
    sort_order: 1
  },
  %{
    name: "120min",
    display_name: "120 Minutes",
    paddle_product_id: "pro_ccc",
    paddle_price_id: "pri_ccc",
    price_cents: 1500,  # $15 (bonus)
    minutes: 120,
    sort_order: 2
  }
]
```

## Core Logic

### Billing Context

```elixir
defmodule PoddyclipBackend.Billing do
  @moduledoc """
  Billing context - plans, subscriptions, usage tracking.
  """

  import Ecto.Query
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Billing.{Plan, MinutePack, ProcessedWebhook}
  alias PoddyclipBackend.Accounts.User

  # --- Plans ---

  def list_active_plans do
    Plan |> where(active: true) |> order_by(:sort_order) |> Repo.all()
  end

  def get_plan!(id), do: Repo.get!(Plan, id)
  def get_plan_by_name(name), do: Repo.get_by(Plan, name: name)
  def get_plan_by_paddle_price_id(price_id), do: Repo.get_by(Plan, paddle_price_id: price_id)

  # --- Minute Packs ---

  def list_active_minute_packs do
    MinutePack |> where(active: true) |> order_by(:sort_order) |> Repo.all()
  end

  def get_minute_pack_by_paddle_price_id(price_id) do
    Repo.get_by(MinutePack, paddle_price_id: price_id)
  end

  # --- User Lookups ---

  def get_user_by_subscription_id(subscription_id) do
    Repo.get_by(User, paddle_subscription_id: subscription_id)
  end

  # --- Usage (with race condition protection) ---

  @doc """
  Calculate available minutes for a user.
  Preloads plan if not already loaded.
  """
  def available_minutes(user) do
    user = Repo.preload(user, :plan)
    available_minutes_loaded(user)
  end

  # Pure helper - use inside transactions where user.plan is already loaded
  defp available_minutes_loaded(user) do
    subscription_remaining = max(0, user.plan.included_minutes - user.subscription_minutes_used)
    subscription_remaining + user.purchased_minutes
  end

  @doc """
  Quick check if user likely has enough minutes.

  WARNING: This uses non-locked data - only for UX feedback (show error early).
  For actual entitlement, use deduct_minutes/2 which is transactional and race-safe.
  """
  def has_minutes_for?(user, duration_minutes) do
    available_minutes(user) >= duration_minutes
  end

  @doc """
  Atomically check and deduct minutes. Returns {:ok, user} or {:error, :insufficient_minutes}.
  Uses SELECT FOR UPDATE to prevent race conditions.
  """
  def deduct_minutes(user_id, minutes_used) when is_integer(user_id) do
    Repo.transaction(fn ->
      # Lock the user row with plan preloaded
      user =
        from(u in User,
          where: u.id == ^user_id,
          lock: "FOR UPDATE",
          preload: [:plan]
        )
        |> Repo.one!()

      # Use pure helper - no additional queries
      available = available_minutes_loaded(user)

      if available < minutes_used do
        Repo.rollback(:insufficient_minutes)
      end

      # Calculate deduction
      sub_remaining = max(0, user.plan.included_minutes - user.subscription_minutes_used)

      {new_sub_used, new_purchased} =
        cond do
          minutes_used <= sub_remaining ->
            # All from subscription
            {user.subscription_minutes_used + minutes_used, user.purchased_minutes}

          sub_remaining > 0 ->
            # Some subscription, rest from purchased
            from_purchased = minutes_used - sub_remaining
            {user.plan.included_minutes, user.purchased_minutes - from_purchased}

          true ->
            # All from purchased
            {user.subscription_minutes_used, user.purchased_minutes - minutes_used}
        end

      user
      |> Ecto.Changeset.change(
        subscription_minutes_used: new_sub_used,
        purchased_minutes: new_purchased
      )
      |> Repo.update!()
    end)
  end

  @doc """
  Add purchased minutes to user (called from webhook).
  Uses transaction for idempotency check.
  """
  def add_purchased_minutes(user, minutes, event_id, resource_key) do
    Repo.transaction(fn ->
      if webhook_processed?(event_id, resource_key) do
        Repo.rollback(:already_processed)
      end

      mark_webhook_processed(event_id, resource_key, "transaction.completed")

      user
      |> Ecto.Changeset.change(purchased_minutes: user.purchased_minutes + minutes)
      |> Repo.update!()
    end)
  end

  # --- Subscription Status ---

  @doc """
  Check if user has usable subscription access.
  Handles both active and canceled-but-not-expired states.
  Use this for entitlement checks - cron handles cleanup.
  """
  def has_subscription_access?(user) do
    case user.subscription_status do
      "active" ->
        true

      "canceled" ->
        # Still has access until period ends
        not is_nil(user.current_period_ends_at) and
          DateTime.compare(DateTime.utc_now(), user.current_period_ends_at) == :lt

      _ ->
        false
    end
  end

  @doc """
  Get effective plan for user (respects canceled-but-not-expired).
  """
  def effective_plan(user) do
    user = Repo.preload(user, :plan)

    if has_subscription_access?(user) do
      user.plan
    else
      # Fallback to free plan
      get_plan_by_name("free")
    end
  end

  @doc """
  Update subscription from webhook data.
  """
  def update_subscription(user, attrs, event_id, resource_key \\ "subscription") do
    Repo.transaction(fn ->
      if webhook_processed?(event_id, resource_key) do
        Repo.rollback(:already_processed)
      end

      mark_webhook_processed(event_id, resource_key, attrs[:event_type] || "subscription.updated")

      user
      |> Ecto.Changeset.change(Map.delete(attrs, :event_type))
      |> Repo.update!()
    end)
  end

  # --- Webhook Idempotency ---

  defp webhook_processed?(event_id, resource_key) do
    Repo.exists?(
      from p in ProcessedWebhook,
        where: p.event_id == ^event_id and p.resource_key == ^resource_key
    )
  end

  defp mark_webhook_processed(event_id, resource_key, event_type) do
    %ProcessedWebhook{}
    |> Ecto.Changeset.change(event_id: event_id, resource_key: resource_key, event_type: event_type)
    |> Repo.insert!()
  end
end
```

### Check Before Processing

```elixir
# In job creation / processing flow
def create_job(user, file, opts) do
  duration = estimate_duration(file)  # or actual duration after probe

  # Quick UX check (not authoritative - just for early feedback)
  unless Billing.has_minutes_for?(user, duration) do
    {:error, :insufficient_minutes}
  end

  # Create job, start processing...

  # After successful processing, atomically deduct (this is the real check)
  case Billing.deduct_minutes(user.id, actual_duration) do
    {:ok, _user} ->
      {:ok, job}

    {:error, :insufficient_minutes} ->
      # Edge case: user ran out between check and completion
      # Job succeeded but can't charge - decide policy:
      # Option A: Allow it (be generous)
      # Option B: Mark job but don't let them download
      {:ok, job}  # Being generous
  end
end
```

## Paddle Webhooks

### Endpoint

```elixir
# lib/poddyclip_backend_web/controllers/webhook_controller.ex
defmodule PoddyclipBackendWeb.WebhookController do
  use PoddyclipBackendWeb, :controller
  require Logger

  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Accounts

  def paddle(conn, params) do
    with :ok <- verify_signature(conn),
         :ok <- handle_event(params) do
      json(conn, %{received: true})
    else
      {:error, :invalid_signature} ->
        conn |> put_status(401) |> json(%{error: "Invalid signature"})

      {:error, :already_processed} ->
        # Idempotent - return success
        json(conn, %{received: true})

      {:error, reason} ->
        Logger.error("Webhook error: #{inspect(reason)}")
        conn |> put_status(400) |> json(%{error: "Processing failed"})
    end
  end

  defp handle_event(%{"event_id" => event_id, "event_type" => "subscription.created"} = event) do
    %{
      "data" => %{
        "id" => subscription_id,
        "customer_id" => customer_id,
        "status" => status,
        "custom_data" => %{"user_id" => user_id},
        "items" => [%{"price" => %{"id" => price_id}} | _],
        "current_billing_period" => %{
          "starts_at" => starts_at,
          "ends_at" => ends_at
        }
      }
    } = event

    # Guard against missing data - webhooks should never crash
    with %Plan{} = plan <- Billing.get_plan_by_paddle_price_id(price_id),
         %User{} = user <- Accounts.get_user(user_id) do
      case Billing.update_subscription(user, %{
        event_type: "subscription.created",
        paddle_customer_id: customer_id,
        paddle_subscription_id: subscription_id,
        plan_id: plan.id,
        subscription_status: status,
        subscription_minutes_used: 0,
        current_period_starts_at: parse_datetime(starts_at),
        current_period_ends_at: parse_datetime(ends_at),
        scheduled_cancel_at: nil
      }, event_id) do
        {:ok, _} ->
          Logger.info("Subscription created for user #{user_id}, plan: #{plan.name}")
          :ok
        {:error, :already_processed} ->
          {:error, :already_processed}
      end
    else
      nil ->
        Logger.warning("subscription.created: user #{user_id} or plan not found, skipping")
        :ok
    end
  end

  defp handle_event(%{"event_id" => event_id, "event_type" => "subscription.updated"} = event) do
    %{
      "data" => %{
        "id" => subscription_id,
        "status" => status,
        "scheduled_change" => scheduled_change,
        "items" => [%{"price" => %{"id" => price_id}} | _],
        "current_billing_period" => %{
          "starts_at" => starts_at,
          "ends_at" => ends_at
        }
      }
    } = event

    # Guard against missing data
    with %User{} = user <- Billing.get_user_by_subscription_id(subscription_id),
         %Plan{} = plan <- Billing.get_plan_by_paddle_price_id(price_id) do
      # Check if this is a new billing period (reset usage)
      new_period_start = parse_datetime(starts_at)
      reset_usage = is_nil(user.current_period_starts_at) or
        DateTime.compare(new_period_start, user.current_period_starts_at) == :gt

      # Check for scheduled cancellation
      scheduled_cancel_at =
        case scheduled_change do
          %{"action" => "cancel", "effective_at" => effective_at} ->
            parse_datetime(effective_at)
          _ ->
            nil
        end

      attrs = %{
        event_type: "subscription.updated",
        plan_id: plan.id,
        subscription_status: status,
        current_period_starts_at: new_period_start,
        current_period_ends_at: parse_datetime(ends_at),
        scheduled_cancel_at: scheduled_cancel_at
      }

      attrs = if reset_usage, do: Map.put(attrs, :subscription_minutes_used, 0), else: attrs

      case Billing.update_subscription(user, attrs, event_id) do
        {:ok, _} ->
          Logger.info("Subscription updated for user #{user.id}, status: #{status}")
          :ok
        {:error, :already_processed} ->
          {:error, :already_processed}
      end
    else
      nil ->
        Logger.warning("subscription.updated: subscription #{subscription_id} not found, skipping")
        :ok
    end
  end

  defp handle_event(%{"event_id" => event_id, "event_type" => "subscription.canceled"} = event) do
    # Note: "canceled" is Paddle's spelling
    %{
      "data" => %{
        "id" => subscription_id,
        "current_billing_period" => %{"ends_at" => ends_at}
      }
    } = event

    # Guard against missing data
    with %User{} = user <- Billing.get_user_by_subscription_id(subscription_id) do
      # Don't downgrade immediately - user paid for the period
      # Just mark as canceled, they keep access until period ends
      case Billing.update_subscription(user, %{
        event_type: "subscription.canceled",
        subscription_status: "canceled",
        current_period_ends_at: parse_datetime(ends_at)
        # Note: Don't change plan_id yet - they keep access until period ends
      }, event_id) do
        {:ok, _} ->
          Logger.info("Subscription canceled for user #{user.id}, access until #{ends_at}")
          :ok
        {:error, :already_processed} ->
          {:error, :already_processed}
      end
    else
      nil ->
        Logger.warning("subscription.canceled: subscription #{subscription_id} not found, skipping")
        :ok
    end
  end

  defp handle_event(%{"event_id" => event_id, "event_type" => "transaction.completed"} = event) do
    # One-time purchase (minute packs)
    %{
      "data" => %{
        "custom_data" => custom_data,
        "items" => items
      }
    } = event

    # Guard - custom_data might be nil or missing user_id
    user_id = get_in(custom_data, ["user_id"])

    with true <- not is_nil(user_id),
         %User{} = user <- Accounts.get_user(user_id) do
      # Process each item - use resource_key for idempotency
      results =
        Enum.map(items, fn %{"price" => %{"id" => price_id}} ->
          case Billing.get_minute_pack_by_paddle_price_id(price_id) do
            nil ->
              # Not a minute pack (maybe subscription transaction)
              :skip

            pack ->
              case Billing.add_purchased_minutes(user, pack.minutes, event_id, price_id) do
                {:ok, _} ->
                  Logger.info("Added #{pack.minutes} minutes to user #{user_id}")
                  :ok

                {:error, :already_processed} ->
                  :already_processed
              end
          end
        end)

      if Enum.all?(results, &(&1 == :already_processed)) do
        {:error, :already_processed}
      else
        :ok
      end
    else
      _ ->
        Logger.warning("transaction.completed: user not found or missing, skipping")
        :ok
    end
  end

  # --- Future events (TODO) ---

  defp handle_event(%{"event_type" => "transaction.refunded"} = _event) do
    # TODO: Subtract minutes if refunding a minute pack
    # TODO: Handle subscription refunds
    Logger.warning("Unhandled event: transaction.refunded")
    :ok
  end

  defp handle_event(%{"event_type" => "subscription.paused"} = _event) do
    # TODO: Pause access but don't downgrade
    Logger.warning("Unhandled event: subscription.paused")
    :ok
  end

  defp handle_event(%{"event_type" => "subscription.past_due"} = _event) do
    # TODO: Maybe restrict features, send reminder
    Logger.warning("Unhandled event: subscription.past_due")
    :ok
  end

  defp handle_event(%{"event_type" => type} = _event) do
    Logger.debug("Unhandled Paddle event: #{type}")
    :ok
  end

  # --- Signature Verification ---

  defp verify_signature(conn) do
    secret = Application.get_env(:poddyclip_backend, :paddle_webhook_secret)

    # Get the Paddle-Signature header
    signature_header = get_req_header(conn, "paddle-signature") |> List.first()

    if is_nil(signature_header) or is_nil(secret) do
      {:error, :invalid_signature}
    else
      # Parse ts and h1 from header: "ts=123;h1=abc..."
      parts =
        signature_header
        |> String.split(";")
        |> Enum.map(&String.split(&1, "=", parts: 2))
        |> Enum.into(%{}, fn [k, v] -> {k, v} end)

      ts = parts["ts"]
      h1 = parts["h1"]

      # Get raw body (need to cache this in a plug)
      raw_body = conn.assigns[:raw_body]

      # Build signed payload
      signed_payload = "#{ts}:#{raw_body}"

      # Compute expected signature
      expected =
        :crypto.mac(:hmac, :sha256, secret, signed_payload)
        |> Base.encode16(case: :lower)

      if Plug.Crypto.secure_compare(expected, h1) do
        :ok
      else
        {:error, :invalid_signature}
      end
    end
  end

  defp parse_datetime(str) when is_binary(str) do
    {:ok, dt, _} = DateTime.from_iso8601(str)
    dt
  end
  defp parse_datetime(nil), do: nil
end
```

### Raw Body Plug (required for signature verification)

```elixir
# lib/poddyclip_backend_web/plugs/raw_body.ex
defmodule PoddyclipBackendWeb.Plugs.RawBody do
  @moduledoc """
  Caches raw request body for webhook signature verification.
  """

  def init(opts), do: opts

  def call(conn, _opts) do
    {:ok, body, conn} = Plug.Conn.read_body(conn)

    conn
    |> Plug.Conn.assign(:raw_body, body)
    |> Plug.Conn.put_private(:raw_body, body)
  end
end
```

### Router

```elixir
# lib/poddyclip_backend_web/router.ex

# Webhook pipeline (no CSRF, cache raw body)
pipeline :webhook do
  plug :accepts, ["json"]
  plug PoddyclipBackendWeb.Plugs.RawBody
end

scope "/webhooks", PoddyclipBackendWeb do
  pipe_through :webhook

  post "/paddle", WebhookController, :paddle
end
```

## Oban Job: Downgrade Expired Subscriptions

```elixir
# lib/poddyclip_backend/workers/downgrade_expired_subscriptions.ex
defmodule PoddyclipBackend.Workers.DowngradeExpiredSubscriptions do
  @moduledoc """
  Runs periodically to downgrade users whose canceled subscriptions have expired.
  """

  use Oban.Worker, queue: :default, max_attempts: 3

  import Ecto.Query
  require Logger

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Accounts.User
  alias PoddyclipBackend.Billing

  @impl Oban.Worker
  def perform(_job) do
    free_plan = Billing.get_plan_by_name("free")
    now = DateTime.utc_now()

    # Find users with canceled subscriptions whose period has ended
    expired_users =
      from(u in User,
        where: u.subscription_status == "canceled",
        where: not is_nil(u.current_period_ends_at),
        where: u.current_period_ends_at < ^now,
        where: u.plan_id != ^free_plan.id
      )
      |> Repo.all()

    Enum.each(expired_users, fn user ->
      user
      |> Ecto.Changeset.change(
        plan_id: free_plan.id,
        paddle_subscription_id: nil,
        subscription_status: "none",
        subscription_minutes_used: 0
      )
      |> Repo.update!()

      Logger.info("Downgraded user #{user.id} to free plan (subscription expired)")
    end)

    Logger.info("Checked #{length(expired_users)} expired subscriptions")
    :ok
  end
end
```

Add to Oban cron:

```elixir
# config/config.exs
{Oban.Plugins.Cron,
 crontab: [
   # ... existing jobs ...
   # Check for expired subscriptions every hour
   {"0 * * * *", PoddyclipBackend.Workers.DowngradeExpiredSubscriptions}
 ]}
```

## Frontend Integration

### Paddle.js Setup

```javascript
// Load Paddle.js
Paddle.Environment.set("sandbox");  // or "production"
Paddle.Initialize({
  token: "live_xxx"  // client-side token from Paddle
});

// Open checkout for subscription
function subscribe(priceId, userEmail, userId) {
  Paddle.Checkout.open({
    items: [{ priceId: priceId, quantity: 1 }],
    customer: { email: userEmail },
    customData: { user_id: userId }
  });
}

// Open checkout for minute pack
function buyMinutes(priceId, userEmail, userId) {
  Paddle.Checkout.open({
    items: [{ priceId: priceId, quantity: 1 }],
    customer: { email: userEmail },
    customData: { user_id: userId }
  });
}
```

### Usage Display Component

```jsx
function UsageDisplay({ user, plan }) {
  const subRemaining = Math.max(0, plan.included_minutes - user.subscription_minutes_used);
  const total = subRemaining + user.purchased_minutes;

  // Guard against division by zero (free plan with 0 included minutes)
  const percent = plan.included_minutes > 0
    ? Math.min(100, (user.subscription_minutes_used / plan.included_minutes) * 100)
    : 0;

  return (
    <div>
      <h3>{plan.display_name} Plan</h3>
      <div class="progress-bar">
        <div style={{ width: `${percent}%` }} />
      </div>
      <p>{subRemaining} subscription + {user.purchased_minutes} purchased = {total} min available</p>
      <button onClick={() => showMinutePacks()}>Buy More Minutes</button>
    </div>
  );
}
```

## Paddle Dashboard Setup

1. **Create Products**
   - Starter Plan (subscription, monthly)
   - Pro Plan (subscription, monthly)
   - 30 Minutes (one-time)
   - 60 Minutes (one-time)
   - 120 Minutes (one-time)

2. **Create Prices** for each product

3. **Configure Webhook**
   - URL: `https://yourdomain.com/webhooks/paddle`
   - Events: `subscription.*`, `transaction.completed`, `transaction.refunded`

4. **Get IDs** and update your plans/minute_packs tables

## Admin Interface (Optional)

For updating plans without deploy:

```elixir
# Simple LiveView admin or use existing admin like Kaffy
def update_plan(plan, attrs) do
  plan
  |> Plan.changeset(attrs)
  |> Repo.update()
end

def update_minute_pack(pack, attrs) do
  pack
  |> MinutePack.changeset(attrs)
  |> Repo.update()
end
```

Note: Changing prices in your DB doesn't change Paddle prices - you must update both.

## Configuration

```elixir
# config/config.exs (or runtime.exs for secrets)
config :poddyclip_backend,
  paddle_environment: "sandbox",  # or "production"
  paddle_api_key: System.get_env("PADDLE_API_KEY"),
  paddle_webhook_secret: System.get_env("PADDLE_WEBHOOK_SECRET"),
  paddle_client_token: System.get_env("PADDLE_CLIENT_TOKEN")
```

## Testing

```elixir
# Use Paddle sandbox mode
# Test cards: https://developer.paddle.com/concepts/payment-methods/credit-debit-card
# 4242 4242 4242 4242 - Success
# 4000 0000 0000 0002 - Decline
```

## Checklist

### Before Development
- [ ] Paddle account approved
- [ ] Products created in Paddle dashboard
- [ ] Webhook secret obtained

### Development
- [ ] Database migrations run
- [ ] Seed plans and minute packs (with Paddle IDs)
- [ ] Webhook signature verification working
- [ ] Webhook idempotency (processed_webhooks table)
- [ ] Race-condition-safe minute deduction
- [ ] Cancellation respects period end
- [ ] Expired subscription downgrade job

### Integration
- [ ] Webhook endpoint deployed
- [ ] Webhook configured in Paddle dashboard
- [ ] Paddle.js integrated in frontend
- [ ] Usage tracking in processing flow

### Testing
- [ ] Test subscription creation (sandbox)
- [ ] Test subscription renewal (usage reset)
- [ ] Test subscription cancellation (keeps access until period end)
- [ ] Test minute pack purchase
- [ ] Test webhook retry (idempotency)
- [ ] Test concurrent processing (race conditions)

### Production
- [ ] Switch to production environment
- [ ] Monitor webhook logs
- [ ] Set up alerts for failed webhooks
