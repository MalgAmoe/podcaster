defmodule PoddyclipBackend.LogShipper.Backend do
  @moduledoc """
  Logger backend that forwards logs to the LogShipper GenServer.

  Add to config:

      config :logger,
        backends: [:console, PoddyclipBackend.LogShipper.Backend]
  """

  @behaviour :gen_event

  @impl true
  def init(__MODULE__) do
    {:ok, %{}}
  end

  def init({__MODULE__, opts}) do
    {:ok, configure(opts)}
  end

  @impl true
  def handle_call({:configure, opts}, state) do
    {:ok, :ok, configure(opts, state)}
  end

  @impl true
  def handle_event({level, _gl, {Logger, message, timestamp, metadata}}, state) do
    if PoddyclipBackend.LogShipper.enabled?() do
      PoddyclipBackend.LogShipper.log(
        level,
        IO.iodata_to_binary(message),
        format_metadata(metadata, timestamp)
      )
    end
    {:ok, state}
  end

  def handle_event(:flush, state) do
    {:ok, state}
  end

  def handle_event(_, state) do
    {:ok, state}
  end

  @impl true
  def handle_info(_, state) do
    {:ok, state}
  end

  @impl true
  def terminate(_reason, _state) do
    :ok
  end

  @impl true
  def code_change(_old, state, _extra) do
    {:ok, state}
  end

  defp configure(opts, _state \\ %{}) do
    %{level: Keyword.get(opts, :level, :info)}
  end

  defp format_metadata(metadata, {{year, month, day}, {hour, minute, second, micro}}) do
    timestamp =
      NaiveDateTime.new!(year, month, day, hour, minute, second, micro * 1000)
      |> NaiveDateTime.to_iso8601()

    metadata
    |> Keyword.put(:timestamp, timestamp)
    |> Enum.filter(fn {_k, v} -> loggable?(v) end)
    |> Map.new(fn {k, v} -> {k, sanitize(v)} end)
  end

  defp loggable?(v) when is_binary(v), do: true
  defp loggable?(v) when is_number(v), do: true
  defp loggable?(v) when is_atom(v), do: true
  defp loggable?(v) when is_boolean(v), do: true
  defp loggable?(_), do: false

  defp sanitize(v) when is_atom(v), do: Atom.to_string(v)
  defp sanitize(v), do: v
end
