defmodule PoddyclipBackendWeb.PolarWebhookController do
  @moduledoc """
  Handles webhooks from Polar.sh for subscription and order management.

  ## Supported Events

  ### Subscriptions
  - `subscription.created` - Initial subscription record created
  - `subscription.active` - Subscription is now active (payment successful)
  - `subscription.updated` - Subscription modified or renewed
  - `subscription.canceled` - User canceled (still has access until period ends)
  - `subscription.uncanceled` - User reactivated a cancelled subscription
  - `subscription.past_due` - Payment failed, subscription in grace period
  - `subscription.revoked` - Access removed immediately (payment failed, etc.)

  ### Orders
  - `order.paid` - One-time purchase payment confirmed (e.g., minute packs)
  - `order.refunded` - Order was refunded (remove minute pack)

  ## Signature Verification

  All webhooks are verified using the Standard Webhooks spec before processing.
  Events are tracked for idempotency to handle webhook retries safely.
  """
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.{Billing, Polar}

  require Logger

  @doc """
  Handles incoming Polar webhooks.

  Verifies the signature, checks for duplicate events, then routes to
  the appropriate handler based on event type.
  """
  def handle(conn, params) do
    # In tests, raw_body may not be available through CacheBodyReader,
    # so we fall back to re-encoding the params
    raw_body = conn.assigns[:raw_body] || Jason.encode!(params)
    headers = lowercase_headers(conn.req_headers)
    secret = Application.get_env(:poddyclip_backend, :polar_webhook_secret)

    cond do
      is_nil(secret) or secret == "" ->
        Logger.error("Polar webhook received but POLAR_WEBHOOK_SECRET not configured")
        send_error(conn, 500, "Webhook secret not configured")

      raw_body == "" ->
        send_error(conn, 400, "Empty request body")

      true ->
        case Polar.verify_signature(raw_body, headers, secret) do
          {:ok, payload} ->
            process_webhook(conn, payload, headers)

          {:error, :timestamp_too_old} ->
            send_error(conn, 401, "Webhook timestamp too old")

          {:error, :invalid_signature} ->
            send_error(conn, 401, "Invalid signature")

          {:error, {:missing_header, header}} ->
            send_error(conn, 400, "Missing header: #{header}")

          {:error, reason} ->
            Logger.error("Webhook verification failed: #{inspect(reason)}")
            send_error(conn, 401, "Signature verification failed")
        end
    end
  end

  defp process_webhook(conn, %{"type" => type, "data" => data} = payload, headers) do
    # Use webhook-id header (Standard Webhooks spec) or fall back to payload
    event_id = get_event_id(payload, headers)

    if Billing.webhook_processed?(event_id) do
      Logger.info("Skipping duplicate webhook: #{event_id}")
      json(conn, %{status: "already_processed"})
    else
      result = handle_event(type, data)
      Billing.mark_webhook_processed(event_id, type)
      Logger.info("Processed webhook #{type}: #{event_id}")
      json(conn, result)
    end
  end

  defp process_webhook(conn, payload, _headers) do
    Logger.warning("Invalid webhook payload: #{inspect(payload)}")
    send_error(conn, 400, "Invalid payload format")
  end

  # Extract event ID - prefer webhook-id header (Standard Webhooks spec)
  defp get_event_id(payload, headers) do
    headers["webhook-id"] ||
      payload["id"] ||
      payload["event_id"] ||
      "unknown_#{System.unique_integer([:positive])}"
  end

  # ----- Event Handlers -----

  defp handle_event("subscription.created", data) do
    # Subscription created but not yet active (waiting for payment)
    Logger.info("Subscription created: #{inspect(data["id"])}")
    %{status: "ok"}
  end

  defp handle_event("subscription.active", data) do
    # Subscription is now active - activate pro plan
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]
    customer_id = get_customer_id(subscription)
    product = get_product(subscription)

    Logger.info("Subscription active: #{subscription_id}, customer: #{customer_id}")

    # Find the user - they may have been linked during checkout
    user = find_user_for_subscription(subscription)

    if user do
      plan = get_plan_for_product(product)
      period_end = parse_period_end(subscription)

      case Billing.update_subscription(user, %{
             plan_id: plan.id,
             seconds_available: plan.seconds,
             seconds_allocated: plan.seconds,
             subscription_status: "active",
             polar_customer_id: customer_id,
             polar_subscription_id: subscription_id,
             current_period_ends_at: period_end
           }) do
        {:ok, _user} ->
          Logger.info("User #{user.id} upgraded to #{plan.name}")
          %{status: "ok"}

        {:error, reason} ->
          Logger.error("Failed to update user subscription: #{inspect(reason)}")
          %{status: "error", message: "Failed to update user"}
      end
    else
      Logger.warning("No user found for subscription #{subscription_id}")
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("subscription.updated", data) do
    # Subscription was modified (plan change, renewal, etc.)
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]

    user = Billing.get_user_by_subscription_id(subscription_id)

    if user do
      period_end = parse_period_end(subscription)
      old_period_end = user.current_period_ends_at

      # Check if this is a renewal (new period started)
      is_renewal = period_end && old_period_end && DateTime.compare(period_end, old_period_end) == :gt

      updates =
        if is_renewal do
          Logger.info("Subscription renewed for user #{user.id}")
          # Reset seconds on renewal
          user = PoddyclipBackend.Repo.preload(user, :plan)
          seconds = if user.plan, do: user.plan.seconds, else: user.seconds_available
          %{current_period_ends_at: period_end, seconds_available: seconds, seconds_allocated: seconds}
        else
          %{current_period_ends_at: period_end}
        end

      Billing.update_subscription(user, updates)
      %{status: "ok", renewed: is_renewal}
    else
      Logger.warning("No user found for subscription update: #{subscription_id}")
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("subscription.canceled", data) do
    # User canceled - they keep access until period ends
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]

    user = Billing.get_user_by_subscription_id(subscription_id)

    if user do
      Logger.info("Subscription canceled for user #{user.id}")
      Billing.update_subscription(user, %{subscription_status: "cancelled"})
      %{status: "ok"}
    else
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("subscription.uncanceled", data) do
    # User reactivated a cancelled subscription before period ended
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]

    user = Billing.get_user_by_subscription_id(subscription_id)

    if user do
      Logger.info("Subscription uncanceled for user #{user.id}")
      Billing.update_subscription(user, %{subscription_status: "active"})
      %{status: "ok"}
    else
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("subscription.past_due", data) do
    # Payment failed - subscription in grace period (not yet revoked)
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]

    user = Billing.get_user_by_subscription_id(subscription_id)

    if user do
      Logger.warning("Subscription past due for user #{user.id}")
      # Keep access but mark status - user needs to fix payment
      Billing.update_subscription(user, %{subscription_status: "past_due"})
      %{status: "ok"}
    else
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("subscription.revoked", data) do
    # Access removed immediately (payment failed, refund, etc.)
    subscription = data["subscription"] || data
    subscription_id = subscription["id"]

    user = Billing.get_user_by_subscription_id(subscription_id)

    if user do
      Logger.info("Subscription revoked for user #{user.id}")
      free_plan = Billing.get_or_create_free_plan()

      Billing.update_subscription(user, %{
        plan_id: free_plan.id,
        subscription_status: "none",
        polar_subscription_id: nil,
        current_period_ends_at: DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)
        # Keep seconds_available as is - don't take away remaining time
      })

      %{status: "ok"}
    else
      %{status: "ok", warning: "user_not_found"}
    end
  end

  defp handle_event("order.paid", data) do
    # One-time purchase payment confirmed - check if it's a minute pack
    order = data["order"] || data
    order_id = order["id"]
    product = get_order_product(order)
    product_id = product["id"]
    customer = order["customer"] || %{}
    customer_external_id = customer["external_id"]

    # Check metadata for reference_id (set during checkout link redirect)
    # This is our primary way to identify the user for checkout links
    metadata = order["metadata"] || %{}
    reference_id = metadata["reference_id"]

    minute_pack_product_id = Polar.minute_pack_product_id()

    # Try reference_id from metadata first (set via checkout link),
    # then fall back to customer external_id (set via Checkout API)
    user_id_source = reference_id || customer_external_id

    cond do
      is_nil(minute_pack_product_id) ->
        Logger.info("Minute pack product not configured, ignoring order")
        %{status: "ok", ignored: true}

      product_id != minute_pack_product_id ->
        Logger.info("Order is not for minute pack product, ignoring",
          order_product_id: product_id,
          expected_product_id: minute_pack_product_id
        )
        %{status: "ok", ignored: true}

      is_nil(user_id_source) ->
        Logger.warning("Minute pack order has no user identifier (metadata.reference_id or customer.external_id)",
          order_id: order_id
        )
        %{status: "ok", warning: "no_user_identifier"}

      true ->
        # Find user by external_id or metadata.user_id
        # Parse string to integer since Polar returns it as a string
        user_id = parse_user_id(user_id_source)

        case user_id && PoddyclipBackend.Accounts.get_user(user_id) do
          nil ->
            Logger.warning("User not found for minute pack order",
              order_id: order_id,
              user_id_source: user_id_source
            )
            %{status: "ok", warning: "user_not_found"}

          user ->
            case Billing.create_minute_pack(user.id, order_id) do
              {:ok, pack} ->
                Logger.info("Minute pack created from order",
                  user_id: user.id,
                  pack_id: pack.id,
                  order_id: order_id
                )
                %{status: "ok"}

              {:error, reason} ->
                Logger.error("Failed to create minute pack",
                  user_id: user.id,
                  order_id: order_id,
                  error: inspect(reason)
                )
                %{status: "error", message: "Failed to create minute pack"}
            end
        end
    end
  end

  defp handle_event("order.refunded", data) do
    # Order was refunded - delete the minute pack if it exists
    order = data["order"] || data
    order_id = order["id"]

    Logger.info("Order refunded webhook received", order_id: order_id)

    case Billing.delete_minute_pack_by_order_id(order_id) do
      {:ok, _pack} ->
        Logger.info("Minute pack deleted due to refund", order_id: order_id)
        %{status: "ok"}

      {:error, :not_found} ->
        Logger.info("No minute pack found for refunded order", order_id: order_id)
        %{status: "ok", warning: "pack_not_found"}

      {:error, reason} ->
        Logger.error("Failed to delete minute pack on refund",
          order_id: order_id,
          error: inspect(reason)
        )
        %{status: "error", message: "Failed to delete minute pack"}
    end
  end

  defp handle_event(type, _data) do
    Logger.info("Ignoring unhandled webhook type: #{type}")
    %{status: "ok", ignored: true}
  end

  defp get_order_product(order) do
    # Order can have product directly or nested in items
    order["product"] ||
      get_in(order, ["items", Access.at(0), "product"]) ||
      %{}
  end

  # ----- Helpers -----

  defp parse_user_id(external_id) when is_binary(external_id) do
    case Integer.parse(external_id) do
      {id, ""} -> id
      _ -> nil
    end
  end

  defp parse_user_id(external_id) when is_integer(external_id), do: external_id
  defp parse_user_id(_), do: nil

  defp lowercase_headers(headers) do
    headers
    |> Enum.map(fn {k, v} -> {String.downcase(k), v} end)
    |> Map.new()
  end

  defp send_error(conn, status, message) do
    conn
    |> put_status(status)
    |> json(%{error: message})
  end

  defp get_customer_id(subscription) do
    subscription["customer_id"] ||
      get_in(subscription, ["customer", "id"])
  end

  defp get_product(subscription) do
    subscription["product"] ||
      get_in(subscription, ["price", "product"]) ||
      %{}
  end

  defp find_user_for_subscription(subscription) do
    subscription_id = subscription["id"]
    customer_id = get_customer_id(subscription)
    customer_email = get_in(subscription, ["customer", "email"])

    # Try finding by subscription ID first (if already linked),
    # then by customer ID, then by email as last resort for initial activation.
    # Email lookup is safe here because Polar verifies the checkout email
    # matches the customer, and this only matters for the first webhook
    # before polar_customer_id is stored on the user.
    Billing.get_user_by_subscription_id(subscription_id) ||
      (customer_id && Billing.get_user_by_customer_id(customer_id)) ||
      (customer_email && PoddyclipBackend.Accounts.get_user_by_email(customer_email))
  end

  defp get_plan_for_product(product) do
    product_id = product["id"]

    # Try to find by Polar product ID
    case PoddyclipBackend.Repo.get_by(Billing.Plan, polar_product_id: product_id) do
      nil ->
        # Fall back to munch plan
        Billing.get_plan_by_name("munch") || Billing.get_or_create_free_plan()

      plan ->
        plan
    end
  end

  defp parse_period_end(subscription) do
    period_end =
      subscription["current_period_end"] ||
        subscription["currentPeriodEnd"]

    case period_end do
      nil ->
        nil

      ts when is_integer(ts) ->
        DateTime.from_unix!(ts) |> DateTime.truncate(:second)

      ts when is_binary(ts) ->
        case DateTime.from_iso8601(ts) do
          {:ok, dt, _} -> DateTime.truncate(dt, :second)
          _ -> nil
        end

      _ ->
        nil
    end
  end
end
