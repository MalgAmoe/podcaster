defmodule PoddyclipBackend.RateLimiter do
  @moduledoc """
  Simple ETS-based rate limiter for data exports.
  Auto-cleans expired entries on each check.
  """

  use GenServer

  @table :data_export_rate_limit
  @limit_seconds 3600  # 1 hour

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, [], name: __MODULE__)
  end

  @doc """
  Check if user can export data.
  Returns :ok or {:error, seconds_remaining}
  """
  def check_export(user_id) do
    cleanup_expired()

    case :ets.lookup(@table, user_id) do
      [{^user_id, last_export}] ->
        elapsed = System.system_time(:second) - last_export
        if elapsed >= @limit_seconds do
          :ok
        else
          {:error, @limit_seconds - elapsed}
        end

      [] ->
        :ok
    end
  end

  @doc """
  Record that user exported data.
  """
  def record_export(user_id) do
    :ets.insert(@table, {user_id, System.system_time(:second)})
    :ok
  end

  # GenServer callbacks

  @impl true
  def init(_) do
    :ets.new(@table, [:set, :public, :named_table])
    {:ok, %{}}
  end

  # Clean up entries older than limit
  defp cleanup_expired do
    cutoff = System.system_time(:second) - @limit_seconds

    :ets.select_delete(@table, [
      {{:"$1", :"$2"}, [{:<, :"$2", cutoff}], [true]}
    ])
  end
end
