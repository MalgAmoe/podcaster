defmodule PoddyclipBackend.PreviewGate do
  @moduledoc """
  Guest-only preview shaping for the demo flow.

  This module enforces:
  - a small burst budget per guest over a rolling window
  - a cap on simultaneous guest previews forwarded from Phoenix to Rust
  - a bounded in-memory FIFO wait queue when the demo slot is occupied

  Signed-in users are intentionally not gated here.
  """

  use GenServer

  require Logger

  @timestamps_table :preview_gate_timestamps
  @default_window_ms 15 * 60 * 1000
  @default_burst_limit 3
  @default_busy_retry_ms 10_000
  @default_max_inflight 1
  @default_queue_wait_ms 30_000
  @default_preview_status_ttl_ms 60_000
  @call_timeout_buffer_ms 5_000

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, %{}, name: __MODULE__)
  end

  @doc """
  Try to acquire permission for a guest preview request.

  Returns:
  - `{:ok, token}` when the request may proceed
  - `{:error, :rate_limited, retry_after_ms}` when the guest is over budget
  - `{:error, :preview_busy, retry_after_ms}` when the queued wait times out
  """
  def acquire_guest_preview(user_id, preview_request_id \\ nil, timeout \\ call_timeout_ms())
      when is_integer(user_id) and is_integer(timeout) do
    GenServer.call(__MODULE__, {:acquire_guest_preview, user_id, preview_request_id}, timeout)
  end

  @doc """
  Release a previously acquired guest preview token.
  """
  def release_guest_preview(token) do
    GenServer.call(__MODULE__, {:release_guest_preview, token})
  end

  def register_preview_request(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:register_preview_request, user_id, request_id})
  end

  def mark_preview_processing(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:set_preview_status, user_id, request_id, "processing"})
  end

  def mark_preview_starting(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:set_preview_status, user_id, request_id, "starting"})
  end

  def mark_preview_completed(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:set_preview_status, user_id, request_id, "completed"})
  end

  def mark_preview_failed(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:set_preview_status, user_id, request_id, "failed"})
  end

  def get_preview_status(user_id, request_id)
      when is_integer(user_id) and is_binary(request_id) do
    GenServer.call(__MODULE__, {:get_preview_status, user_id, request_id})
  end

  @doc false
  def reset_for_test do
    GenServer.call(__MODULE__, :reset_for_test)
  end

  @impl true
  def init(_state) do
    :ets.new(@timestamps_table, [:duplicate_bag, :public, :named_table])
    {:ok, empty_state()}
  end

  @impl true
  def handle_call({:acquire_guest_preview, user_id, preview_request_id}, from, state) do
    now_ms = System.system_time(:millisecond)
    timestamps = prune_and_get_timestamps(user_id, now_ms)
    state = maybe_register_preview(state, user_id, preview_request_id)

    cond do
      length(timestamps) >= burst_limit() ->
        oldest = Enum.min(timestamps)
        retry_after_ms = max(window_ms() - (now_ms - oldest), 1)
        {:reply, {:error, :rate_limited, retry_after_ms},
         maybe_set_preview_status(state, user_id, preview_request_id, "failed")}

      map_size(state.active_tokens) < max_inflight() and map_size(state.waiting) == 0 ->
        {token, state} = grant_token(state, user_id, now_ms)
        {:reply, {:ok, token}, state}

      true ->
        state = enqueue_waiter(state, from, user_id, preview_request_id)
        {:noreply, state}
    end
  end

  @impl true
  def handle_call({:release_guest_preview, token}, _from, state) do
    state =
      state
      |> Map.update!(:active_tokens, &Map.delete(&1, token))
      |> maybe_grant_waiters()

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:register_preview_request, user_id, request_id}, _from, state) do
    {:reply, :ok, maybe_register_preview(state, user_id, request_id)}
  end

  @impl true
  def handle_call({:set_preview_status, user_id, request_id, status}, _from, state) do
    {:reply, :ok, maybe_set_preview_status(state, user_id, request_id, status)}
  end

  @impl true
  def handle_call({:get_preview_status, user_id, request_id}, _from, state) do
    case Map.get(state.preview_requests, request_id) do
      %{user_id: ^user_id, status: status} ->
        {:reply, {:ok, %{request_id: request_id, status: status}}, state}

      %{user_id: _other_user_id} ->
        {:reply, {:error, :unauthorized}, state}

      nil ->
        {:reply, {:ok, %{request_id: request_id, status: "pending"}}, state}
    end
  end

  @impl true
  def handle_call(:reset_for_test, _from, state) do
    clear_waiters(state)
    :ets.delete_all_objects(@timestamps_table)
    {:reply, :ok, empty_state()}
  end

  @impl true
  def handle_info({:waiter_timeout, request_id}, state) do
    case pop_waiter(state, request_id) do
      {nil, state} ->
        {:noreply, state}

      {waiter, state} ->
        state = maybe_set_preview_status(state, waiter.user_id, waiter.preview_request_id, "failed")

        Logger.info("Guest preview queue timeout",
          user_id: waiter.user_id,
          inflight: map_size(state.active_tokens),
          queued: map_size(state.waiting)
        )

        GenServer.reply(waiter.from, {:error, :preview_busy, busy_retry_ms()})
        {:noreply, state}
    end
  end

  @impl true
  def handle_info({:cleanup_preview_status, request_id}, state) do
    {:noreply, update_in(state.preview_requests, &Map.delete(&1, request_id))}
  end

  @impl true
  def handle_info({:DOWN, monitor_ref, :process, _pid, _reason}, state) do
    case Map.pop(state.monitor_to_request, monitor_ref) do
      {nil, _} ->
        {:noreply, state}

      {request_id, monitor_to_request} ->
        {waiter, state} = pop_waiter(%{state | monitor_to_request: monitor_to_request}, request_id)

        if waiter do
          Logger.info("Guest preview queue caller disconnected",
            user_id: waiter.user_id,
            inflight: map_size(state.active_tokens),
            queued: map_size(state.waiting)
          )
        end

        {:noreply, state}
    end
  end

  defp empty_state do
    %{
      active_tokens: %{},
      wait_queue: :queue.new(),
      waiting: %{},
      monitor_to_request: %{},
      preview_requests: %{}
    }
  end

  defp enqueue_waiter(state, from, user_id, preview_request_id) do
    waiter_id = make_ref()
    timer_ref = Process.send_after(self(), {:waiter_timeout, waiter_id}, queue_wait_ms())
    monitor_ref = Process.monitor(elem(from, 0))

    Logger.info("Guest preview queued",
      user_id: user_id,
      inflight: map_size(state.active_tokens),
      queued: map_size(state.waiting) + 1
    )

    waiter = %{
      from: from,
      user_id: user_id,
      preview_request_id: preview_request_id,
      timer_ref: timer_ref,
      monitor_ref: monitor_ref
    }

    state
    |> update_in([:wait_queue], &:queue.in(waiter_id, &1))
    |> put_in([:waiting, waiter_id], waiter)
    |> put_in([:monitor_to_request, monitor_ref], waiter_id)
  end

  defp maybe_grant_waiters(state) do
    if map_size(state.active_tokens) < max_inflight() and map_size(state.waiting) > 0 do
      case pop_next_waiter(state) do
        {nil, state} ->
          state

        {request_id, waiter, state} ->
          now_ms = System.system_time(:millisecond)
          {token, state} = grant_token(state, waiter.user_id, now_ms)

          Logger.info("Guest preview dequeued",
            user_id: waiter.user_id,
            inflight: map_size(state.active_tokens),
            queued: map_size(state.waiting)
          )

          cancel_waiter_refs(request_id, waiter)
          GenServer.reply(waiter.from, {:ok, token})

          maybe_grant_waiters(state)
      end
    else
      state
    end
  end

  defp pop_next_waiter(state) do
    case :queue.out(state.wait_queue) do
      {:empty, _queue} ->
        {nil, state}

      {{:value, request_id}, queue} ->
        state = %{state | wait_queue: queue}

        case Map.pop(state.waiting, request_id) do
          {nil, waiting} ->
            pop_next_waiter(%{state | waiting: waiting})

          {waiter, waiting} ->
            state = %{state | waiting: waiting}
            {request_id, waiter, pop_monitor_ref(state, waiter.monitor_ref)}
        end
    end
  end

  defp pop_waiter(state, request_id) do
    case Map.pop(state.waiting, request_id) do
      {nil, waiting} ->
        {nil, %{state | waiting: waiting}}

      {waiter, waiting} ->
        cancel_waiter_refs(request_id, waiter)
        {waiter, %{state | waiting: waiting} |> pop_monitor_ref(waiter.monitor_ref)}
    end
  end

  defp grant_token(state, user_id, now_ms) do
    token = make_ref()
    :ets.insert(@timestamps_table, {user_id, now_ms})
    {token, put_in(state.active_tokens[token], user_id)}
  end

  defp pop_monitor_ref(state, monitor_ref) do
    update_in(state.monitor_to_request, &Map.delete(&1, monitor_ref))
  end

  defp cancel_waiter_refs(_request_id, waiter) do
    Process.cancel_timer(waiter.timer_ref)
    Process.demonitor(waiter.monitor_ref, [:flush])
  end

  defp clear_waiters(state) do
    Enum.each(state.waiting, fn {_request_id, waiter} ->
      cancel_waiter_refs(nil, waiter)
      GenServer.reply(waiter.from, {:error, :preview_busy, busy_retry_ms()})
    end)

    Enum.each(state.preview_requests, fn {_request_id, preview_request} ->
      if preview_request.cleanup_timer_ref do
        Process.cancel_timer(preview_request.cleanup_timer_ref)
      end
    end)
  end

  defp maybe_register_preview(state, _user_id, nil), do: state

  defp maybe_register_preview(state, user_id, request_id) do
    maybe_set_preview_status(state, user_id, request_id, "queued")
  end

  defp maybe_set_preview_status(state, _user_id, nil, _status), do: state

  defp maybe_set_preview_status(state, user_id, request_id, status) do
    previous = Map.get(state.preview_requests, request_id)

    if previous && previous.status == status do
      state
    else
      if previous && previous.cleanup_timer_ref do
        Process.cancel_timer(previous.cleanup_timer_ref)
      end

      cleanup_timer_ref =
        if status in ["completed", "failed"] do
          Process.send_after(self(), {:cleanup_preview_status, request_id}, preview_status_ttl_ms())
        else
          nil
        end

      broadcast_preview_update(request_id, status)

      put_in(state.preview_requests[request_id], %{
        user_id: user_id,
        status: status,
        cleanup_timer_ref: cleanup_timer_ref
      })
    end
  end

  defp broadcast_preview_update(request_id, status) do
    Phoenix.PubSub.broadcast(
      PoddyclipBackend.PubSub,
      "preview:#{request_id}",
      {:preview_updated, %{request_id: request_id, status: status}}
    )
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

  defp queue_wait_ms do
    Application.get_env(:poddyclip_backend, :preview_queue_wait_ms, @default_queue_wait_ms)
  end

  defp preview_status_ttl_ms do
    Application.get_env(
      :poddyclip_backend,
      :preview_status_ttl_ms,
      @default_preview_status_ttl_ms
    )
  end

  defp call_timeout_ms do
    queue_wait_ms() + @call_timeout_buffer_ms
  end
end
