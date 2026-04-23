defmodule PoddyclipBackend.PreviewGateTest do
  use ExUnit.Case, async: false

  alias PoddyclipBackend.PreviewGate

  setup do
    previous_env = %{
      preview_burst_limit: Application.get_env(:poddyclip_backend, :preview_burst_limit),
      preview_rate_window_ms: Application.get_env(:poddyclip_backend, :preview_rate_window_ms),
      preview_busy_retry_ms: Application.get_env(:poddyclip_backend, :preview_busy_retry_ms),
      preview_max_inflight: Application.get_env(:poddyclip_backend, :preview_max_inflight),
      preview_queue_wait_ms: Application.get_env(:poddyclip_backend, :preview_queue_wait_ms)
    }

    on_exit(fn ->
      Enum.each(previous_env, fn {key, value} ->
        if is_nil(value) do
          Application.delete_env(:poddyclip_backend, key)
        else
          Application.put_env(:poddyclip_backend, key, value)
        end
      end)

      PreviewGate.reset_for_test()
    end)

    :ok
  end

  test "rate limits after the configured guest burst" do
    Application.put_env(:poddyclip_backend, :preview_burst_limit, 3)
    Application.put_env(:poddyclip_backend, :preview_rate_window_ms, 900_000)

    {:ok, token1} = PreviewGate.acquire_guest_preview(123)
    PreviewGate.release_guest_preview(token1)
    {:ok, token2} = PreviewGate.acquire_guest_preview(123)
    PreviewGate.release_guest_preview(token2)
    {:ok, token3} = PreviewGate.acquire_guest_preview(123)
    PreviewGate.release_guest_preview(token3)

    assert {:error, :rate_limited, retry_after_ms} = PreviewGate.acquire_guest_preview(123)
    assert is_integer(retry_after_ms)
    assert retry_after_ms > 0
  end

  test "queues a guest preview and grants it when a slot is released" do
    Application.put_env(:poddyclip_backend, :preview_max_inflight, 1)
    Application.put_env(:poddyclip_backend, :preview_queue_wait_ms, 500)

    {:ok, token} = PreviewGate.acquire_guest_preview(123)

    task =
      Task.async(fn ->
        PreviewGate.acquire_guest_preview(456, 2_000)
      end)

    assert Task.yield(task, 50) == nil

    :ok = PreviewGate.release_guest_preview(token)

    assert {:ok, queued_token} = Task.await(task, 2_000)
    assert is_reference(queued_token)
  end

  test "returns preview_busy when a queued preview waits too long" do
    Application.put_env(:poddyclip_backend, :preview_max_inflight, 1)
    Application.put_env(:poddyclip_backend, :preview_queue_wait_ms, 50)
    Application.put_env(:poddyclip_backend, :preview_busy_retry_ms, 10_000)

    {:ok, token} = PreviewGate.acquire_guest_preview(123)

    assert {:error, :preview_busy, 10_000} = PreviewGate.acquire_guest_preview(456, 1_000)

    :ok = PreviewGate.release_guest_preview(token)
  end

  test "removes disconnected callers from the queue" do
    Application.put_env(:poddyclip_backend, :preview_max_inflight, 1)
    Application.put_env(:poddyclip_backend, :preview_queue_wait_ms, 500)

    {:ok, token} = PreviewGate.acquire_guest_preview(123)

    pid =
      spawn(fn ->
        PreviewGate.acquire_guest_preview(456, 2_000)
      end)

    Process.sleep(50)
    Process.exit(pid, :kill)
    Process.sleep(50)

    :ok = PreviewGate.release_guest_preview(token)

    assert {:ok, next_token} = PreviewGate.acquire_guest_preview(789, 500)
    assert is_reference(next_token)

    :ok = PreviewGate.release_guest_preview(next_token)
  end
end
