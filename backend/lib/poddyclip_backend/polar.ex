defmodule PoddyclipBackend.Polar do
  @moduledoc """
  Polar.sh webhook signature verification.

  Polar uses the Standard Webhooks spec (https://www.standardwebhooks.com/)
  which uses HMAC-SHA256 for signature verification.

  ## Headers

  - `webhook-id`: Unique identifier for the webhook event
  - `webhook-timestamp`: Unix timestamp when the webhook was sent
  - `webhook-signature`: Space-separated list of signatures (e.g., "v1,sig1 v1,sig2")

  ## Signature Format

  The message to sign is: `{webhook_id}.{webhook_timestamp}.{body}`
  The signature is base64-encoded HMAC-SHA256.
  """

  @doc """
  Verifies a webhook signature.

  ## Parameters

  - `raw_body`: The raw request body as a string
  - `headers`: Map of request headers (lowercased keys)
  - `secret`: The webhook secret (with optional "whsec_" prefix)

  ## Returns

  - `{:ok, payload}` if signature is valid
  - `{:error, reason}` if signature is invalid

  ## Example

      iex> verify_signature(body, headers, "whsec_abc123...")
      {:ok, %{"type" => "subscription.active", ...}}
  """
  def verify_signature(raw_body, headers, secret) do
    with {:ok, webhook_id} <- get_header(headers, "webhook-id"),
         {:ok, timestamp} <- get_header(headers, "webhook-timestamp"),
         {:ok, signatures} <- get_header(headers, "webhook-signature"),
         :ok <- verify_timestamp(timestamp),
         :ok <- verify_signatures(raw_body, webhook_id, timestamp, signatures, secret) do
      {:ok, Jason.decode!(raw_body)}
    end
  end

  defp get_header(headers, key) do
    case Map.get(headers, key) do
      nil -> {:error, {:missing_header, key}}
      value when is_list(value) -> {:ok, List.first(value)}
      value -> {:ok, value}
    end
  end

  # Verify timestamp is within 5 minutes to prevent replay attacks
  defp verify_timestamp(timestamp_str) do
    case Integer.parse(timestamp_str) do
      {timestamp, ""} ->
        now = System.system_time(:second)
        age = now - timestamp

        if age < 0 or age > 300 do
          {:error, :timestamp_too_old}
        else
          :ok
        end

      _ ->
        {:error, :invalid_timestamp}
    end
  end

  defp verify_signatures(body, webhook_id, timestamp, signatures_str, secret) do
    secret_bytes = decode_secret(secret)

    # Build the signed payload
    signed_payload = "#{webhook_id}.#{timestamp}.#{body}"

    # Compute expected signature
    expected_sig =
      :crypto.mac(:hmac, :sha256, secret_bytes, signed_payload)
      |> Base.encode64()

    # Parse signatures from header (format: "v1,sig1 v1,sig2")
    provided_sigs =
      signatures_str
      |> String.split(" ")
      |> Enum.map(fn sig ->
        case String.split(sig, ",", parts: 2) do
          ["v1", signature] -> signature
          _ -> nil
        end
      end)
      |> Enum.reject(&is_nil/1)

    # Check if any provided signature matches
    if Enum.any?(provided_sigs, &secure_compare(&1, expected_sig)) do
      :ok
    else
      {:error, :invalid_signature}
    end
  end

  defp decode_secret(secret) do
    # Polar SDK does: Buffer.from(secret, "utf-8").toString("base64")
    # Then Standard Webhooks decodes that base64.
    # Net result: use the raw UTF-8 bytes of the secret string directly.
    secret
  end

  # Constant-time comparison to prevent timing attacks
  defp secure_compare(a, b) when byte_size(a) == byte_size(b) do
    :crypto.hash_equals(a, b)
  end

  defp secure_compare(_, _), do: false

  # ----- Checkout & Portal URLs -----

  @doc """
  Generates a Polar checkout URL for a user to upgrade to a plan.

  The URL includes:
  - Product ID from the plan
  - Customer email prefilled
  - Success URL back to /account

  ## Parameters

  - `user`: The user upgrading
  - `plan`: The plan with a polar_product_id

  ## Returns

  The checkout URL as a string, or nil if the plan has no polar_product_id.
  """
  def checkout_url(user, _plan) do
    checkout_link_id = Application.get_env(:poddyclip_backend, :polar_checkout_link_id)

    if checkout_link_id do
      api_host = polar_api_host()

      query = URI.encode_query(%{"customer_email" => user.email})

      "https://#{api_host}/v1/checkout-links/#{checkout_link_id}/redirect?#{query}"
    else
      nil
    end
  end

  @doc """
  Generates a Polar customer portal URL for subscription management.

  Users can use this to:
  - Update payment method
  - Cancel subscription
  - View billing history

  ## Parameters

  - `user`: The user (must have polar_customer_id)

  ## Returns

  The portal URL as a string, or nil if user has no polar_customer_id.
  """
  def customer_portal_url(user) do
    customer_id = user.polar_customer_id

    if customer_id do
      host = polar_host()
      org = polar_organization()
      "https://#{host}/#{org}/portal?customer_id=#{customer_id}"
    else
      nil
    end
  end

  defp polar_host do
    Application.get_env(:poddyclip_backend, :polar_host, "polar.sh")
  end

  defp polar_api_host do
    case polar_host() do
      "sandbox.polar.sh" -> "sandbox-api.polar.sh"
      _ -> "api.polar.sh"
    end
  end

  defp polar_organization do
    Application.get_env(:poddyclip_backend, :polar_organization, "munchy-cow")
  end

  defp polar_access_token do
    Application.get_env(:poddyclip_backend, :polar_access_token)
  end

  # ----- API Client -----

  @doc """
  Fetches subscriptions for a customer from Polar API.

  Returns `{:ok, subscriptions}` with a list of subscription objects,
  or `{:error, reason}` on failure.
  """
  def get_customer_subscriptions(customer_id) when is_binary(customer_id) do
    case polar_access_token() do
      nil ->
        {:error, :no_access_token}

      token ->
        url = "https://#{polar_api_host()}/v1/subscriptions?customer_id=#{customer_id}"
        api_get(url, token)
    end
  end

  @doc """
  Fetches a single subscription by ID from Polar API.

  Returns `{:ok, subscription}` or `{:error, reason}`.
  """
  def get_subscription(subscription_id) when is_binary(subscription_id) do
    case polar_access_token() do
      nil ->
        {:error, :no_access_token}

      token ->
        url = "https://#{polar_api_host()}/v1/subscriptions/#{subscription_id}"
        api_get(url, token)
    end
  end

  @doc """
  Fetches all active subscriptions from Polar API.

  Returns `{:ok, subscriptions}` with a list of active subscription objects.
  """
  def list_active_subscriptions do
    case polar_access_token() do
      nil ->
        {:error, :no_access_token}

      token ->
        url = "https://#{polar_api_host()}/v1/subscriptions?active=true&limit=100"
        api_get(url, token)
    end
  end

  @doc """
  Fetches a customer by ID from Polar API.

  Returns `{:ok, customer}` with customer data including email.
  """
  def get_customer(customer_id) when is_binary(customer_id) do
    case polar_access_token() do
      nil ->
        {:error, :no_access_token}

      token ->
        url = "https://#{polar_api_host()}/v1/customers/#{customer_id}"
        api_get(url, token)
    end
  end

  # Internal HTTP GET helper
  defp api_get(url, token) do
    # Ensure inets is started
    :inets.start()
    :ssl.start()

    headers = [
      {~c"Authorization", String.to_charlist("Bearer #{token}")},
      {~c"Accept", ~c"application/json"}
    ]

    case :httpc.request(:get, {String.to_charlist(url), headers}, [], []) do
      {:ok, {{_, 200, _}, _, body}} ->
        case Jason.decode(to_string(body)) do
          {:ok, %{"items" => items}} -> {:ok, items}
          {:ok, data} -> {:ok, data}
          error -> error
        end

      {:ok, {{_, status, _}, _, body}} ->
        {:error, {:http_error, status, to_string(body)}}

      {:error, reason} ->
        {:error, reason}
    end
  end
end
