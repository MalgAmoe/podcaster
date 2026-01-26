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
    # Decode the secret (remove "whsec_" prefix if present)
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
    # Remove "whsec_" prefix if present, then base64 decode
    secret
    |> String.replace_prefix("whsec_", "")
    |> Base.decode64!()
  end

  # Constant-time comparison to prevent timing attacks
  defp secure_compare(a, b) when byte_size(a) == byte_size(b) do
    :crypto.hash_equals(a, b)
  end

  defp secure_compare(_, _), do: false
end
