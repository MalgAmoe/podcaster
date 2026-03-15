defmodule PoddyclipBackend.DemoRateLimiter do
  @moduledoc """
  ETS-based rate limiter for anonymous demo processing.
  Limits each IP to 3 demo attempts per 24 hours.
  """

  use GenServer

  @table :demo_rate_limit
  @max_attempts 3
  @window_seconds 86_400  # 24 hours

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, [], name: __MODULE__)
  end

  @doc """
  Check if an IP can submit a demo.
  Returns :ok or {:error, :rate_limited}
  """
  def check_demo(ip) do
    cleanup_expired(ip)

    case :ets.lookup(@table, ip) do
      [{^ip, timestamps}] ->
        if length(timestamps) >= @max_attempts do
          {:error, :rate_limited}
        else
          :ok
        end

      [] ->
        :ok
    end
  end

  @doc """
  Record a demo attempt for an IP.
  """
  def record_demo(ip) do
    now = System.system_time(:second)

    case :ets.lookup(@table, ip) do
      [{^ip, timestamps}] ->
        :ets.insert(@table, {ip, [now | timestamps]})

      [] ->
        :ets.insert(@table, {ip, [now]})
    end

    :ok
  end

  # GenServer callbacks

  @impl true
  def init(_) do
    :ets.new(@table, [:set, :public, :named_table])
    {:ok, %{}}
  end

  # Remove expired timestamps for a given IP
  defp cleanup_expired(ip) do
    cutoff = System.system_time(:second) - @window_seconds

    case :ets.lookup(@table, ip) do
      [{^ip, timestamps}] ->
        valid = Enum.filter(timestamps, &(&1 > cutoff))

        if valid == [] do
          :ets.delete(@table, ip)
        else
          :ets.insert(@table, {ip, valid})
        end

      [] ->
        :ok
    end
  end
end
