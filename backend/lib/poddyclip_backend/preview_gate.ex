defmodule PoddyclipBackend.PreviewGate do
  @moduledoc """
  Guest-only preview shaping for the demo flow.

  This module enforces:
  - a small burst budget per guest over a rolling window
  - a cap on simultaneous guest previews forwarded from Phoenix to Rust

  Signed-in users are intentionally not gated here.
  """

  use GenServer

  @timestamps_table :preview_gate_timestamps
  @default_window_ms 15 * 60 * 1000
  @default_burst_limit 3
  @default_busy_retry_ms 10_000
  @default_max_inflight 1

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @doc """
  Try to acquire permission for a guest preview request.

  Returns:
  - `{:ok, token}` when the request may proceed
  - `{:error, :rate_limited, retry_after_ms}` when the guest is over budget
  - `{:error, :preview_busy, retry_after_ms}` when demo pressure is too high
  """
  def acquire_guest_preview(user_id) when is_integer(user_id) do
    GenServer.call(__MODULE__, {:acquire_guest_preview, user_id})
  end

  @doc """
  Release a previously acquired guest preview token.
  """
  def release_guest_preview(token) do
    GenServer.call(__MODULE__, {:release_guest_preview, token})
  end

  @impl true
  def init(_state) do
    :ets.new(@timestamps_table, [:duplicate_bag, :public, :named_table])
    {:ok, %{active_tokens: %{}}}
  end

  @impl true
  def handle_call({:acquire_guest_preview, user_id}, _from, state) do
    now_ms = System.system_time(:millisecond)
    timestamps = prune_and_get_timestamps(user_id, now_ms)

    cond do
      length(timestamps) >= burst_limit() ->
        oldest = Enum.min(timestamps)
        retry_after_ms = max(window_ms() - (now_ms - oldest), 1)
        {:reply, {:error, :rate_limited, retry_after_ms}, state}

      map_size(state.active_tokens) >= max_inflight() ->
        {:reply, {:error, :preview_busy, busy_retry_ms()}, state}

      true ->
        token = make_ref()
        :ets.insert(@timestamps_table, {user_id, now_ms})

        {:reply, {:ok, token}, put_in(state.active_tokens[token], user_id)}
    end
  end

  @impl true
  def handle_call({:release_guest_preview, token}, _from, state) do
    {:reply, :ok, %{state | active_tokens: Map.delete(state.active_tokens, token)}}
  end

  defp prune_and_get_timestamps(user_id, now_ms) do
    cutoff = now_ms - window_ms()

    @timestamps_table
    |> :ets.lookup(user_id)
    |> Enum.reduce([], fn {^user_id, timestamp}, acc ->
      if timestamp >= cutoff do
        [timestamp | acc]
      else
        :ets.delete_object(@timestamps_table, {user_id, timestamp})
        acc
      end
    end)
  end

  defp window_ms do
    Application.get_env(:poddyclip_backend, :preview_rate_window_ms, @default_window_ms)
  end

  defp burst_limit do
    Application.get_env(:poddyclip_backend, :preview_burst_limit, @default_burst_limit)
  end

  defp busy_retry_ms do
    Application.get_env(:poddyclip_backend, :preview_busy_retry_ms, @default_busy_retry_ms)
  end

  defp max_inflight do
    Application.get_env(:poddyclip_backend, :preview_max_inflight, @default_max_inflight)
  end
end
