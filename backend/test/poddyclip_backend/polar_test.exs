defmodule PoddyclipBackend.PolarTest do
  use ExUnit.Case, async: true

  alias PoddyclipBackend.Polar

  # Test secret - used directly as HMAC key (production code uses secret string as-is)
  @test_secret "test_webhook_secret_for_testing_12345"

  describe "verify_signature/3" do
    test "verifies valid signature" do
      body = ~s({"type": "test", "data": {}})
      webhook_id = "wh_test_123"
      timestamp = to_string(System.system_time(:second))

      # Compute signature using the secret directly
      signed_payload = "#{webhook_id}.#{timestamp}.#{body}"
      signature = :crypto.mac(:hmac, :sha256, @test_secret, signed_payload) |> Base.encode64()

      headers = %{
        "webhook-id" => webhook_id,
        "webhook-timestamp" => timestamp,
        "webhook-signature" => "v1,#{signature}"
      }

      assert {:ok, payload} = Polar.verify_signature(body, headers, @test_secret)
      assert payload["type"] == "test"
    end

    test "verifies signature with multiple v1 signatures" do
      body = ~s({"type": "multi"})
      webhook_id = "wh_multi_123"
      timestamp = to_string(System.system_time(:second))

      signed_payload = "#{webhook_id}.#{timestamp}.#{body}"
      valid_sig = :crypto.mac(:hmac, :sha256, @test_secret, signed_payload) |> Base.encode64()
      invalid_sig = Base.encode64(:crypto.strong_rand_bytes(32))

      headers = %{
        "webhook-id" => webhook_id,
        "webhook-timestamp" => timestamp,
        # Multiple signatures, valid one is second
        "webhook-signature" => "v1,#{invalid_sig} v1,#{valid_sig}"
      }

      assert {:ok, _} = Polar.verify_signature(body, headers, @test_secret)
    end

    test "rejects invalid signature" do
      body = ~s({"type": "invalid"})
      webhook_id = "wh_invalid"
      timestamp = to_string(System.system_time(:second))

      headers = %{
        "webhook-id" => webhook_id,
        "webhook-timestamp" => timestamp,
        "webhook-signature" => "v1,invalid_signature_here"
      }

      assert {:error, :invalid_signature} = Polar.verify_signature(body, headers, @test_secret)
    end

    test "rejects missing webhook-id header" do
      body = ~s({"type": "test"})

      headers = %{
        "webhook-timestamp" => to_string(System.system_time(:second)),
        "webhook-signature" => "v1,sig"
      }

      assert {:error, {:missing_header, "webhook-id"}} =
               Polar.verify_signature(body, headers, @test_secret)
    end

    test "rejects missing webhook-timestamp header" do
      body = ~s({"type": "test"})

      headers = %{
        "webhook-id" => "wh_123",
        "webhook-signature" => "v1,sig"
      }

      assert {:error, {:missing_header, "webhook-timestamp"}} =
               Polar.verify_signature(body, headers, @test_secret)
    end

    test "rejects missing webhook-signature header" do
      body = ~s({"type": "test"})

      headers = %{
        "webhook-id" => "wh_123",
        "webhook-timestamp" => to_string(System.system_time(:second))
      }

      assert {:error, {:missing_header, "webhook-signature"}} =
               Polar.verify_signature(body, headers, @test_secret)
    end

    test "rejects timestamp older than 5 minutes" do
      body = ~s({"type": "old"})
      webhook_id = "wh_old"
      # 6 minutes ago
      old_timestamp = to_string(System.system_time(:second) - 360)

      signed_payload = "#{webhook_id}.#{old_timestamp}.#{body}"
      signature = :crypto.mac(:hmac, :sha256, @test_secret, signed_payload) |> Base.encode64()

      headers = %{
        "webhook-id" => webhook_id,
        "webhook-timestamp" => old_timestamp,
        "webhook-signature" => "v1,#{signature}"
      }

      assert {:error, :timestamp_too_old} = Polar.verify_signature(body, headers, @test_secret)
    end

    test "uses secret directly as HMAC key" do
      # This test verifies that the secret is used as-is for HMAC,
      # matching how Polar.sh sends webhooks
      secret = "my_plain_secret"

      body = ~s({"type": "plain"})
      webhook_id = "wh_plain"
      timestamp = to_string(System.system_time(:second))

      signed_payload = "#{webhook_id}.#{timestamp}.#{body}"
      signature = :crypto.mac(:hmac, :sha256, secret, signed_payload) |> Base.encode64()

      headers = %{
        "webhook-id" => webhook_id,
        "webhook-timestamp" => timestamp,
        "webhook-signature" => "v1,#{signature}"
      }

      assert {:ok, _} = Polar.verify_signature(body, headers, secret)
    end
  end
end
