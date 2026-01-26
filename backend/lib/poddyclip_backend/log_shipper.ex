defmodule PoddyclipBackend.LogShipper do
  @moduledoc """
  GenServer that ships logs to OpenObserve.

  Batches log events and sends them to the OpenObserve HTTP API.
  Falls back gracefully if OpenObserve is not configured or unavailable.

  ## Configuration

  Configure in runtime.exs:

      config :poddyclip_backend, :openobserve,
        url: "http://localhost:5080",
        user: "admin@poddyclip.local",
        password: "dev",
        org: "default",
        stream: "phoenix"

  ## Usage

  The LogShipper can be used to send structured log events to OpenObserve:

      PoddyclipBackend.LogShipper.log(:info, "Job completed", %{job_id: 123, user_id: 456})

  Or integrate with Logger by adding a custom backend (see LogShipper.Backend).
  """

  use GenServer
  require Logger

  @batch_size 50
  @flush_interval_ms 5_000

  # Sensitive fields that should be redacted from logs
  @sensitive_keys ~w(
    password secret token api_key webhook_secret
    download_url authorization credential signature
  )

  # ----- Client API -----

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @doc """
  Log an event to be shipped to OpenObserve.
  """
  def log(level, message, metadata \\ %{}) do
    if enabled?() do
      GenServer.cast(__MODULE__, {:log, level, message, metadata})
    end
  end

  @doc """
  Check if log shipping is enabled.
  """
  def enabled? do
    config() != nil
  end

  # ----- GenServer Implementation -----

  @impl GenServer
  def init(_opts) do
    case config() do
      nil ->
        IO.puts("[LogShipper] OpenObserve not configured, log shipping disabled")
        {:ok, %{enabled: false, buffer: [], http_client: nil}}

      config ->
        http_client = build_http_client(config)
        IO.puts("[LogShipper] OpenObserve enabled: #{http_client.url}")
        # Schedule first flush
        schedule_flush()
        {:ok, %{
          enabled: true,
          buffer: [],
          config: config,
          http_client: http_client
        }}
    end
  end

  @impl GenServer
  def handle_cast({:log, _level, _message, _metadata}, %{enabled: false} = state) do
    {:noreply, state}
  end

  def handle_cast({:log, level, message, metadata}, state) do
    event = %{
      _timestamp: :os.system_time(:microsecond),
      level: to_string(level),
      message: message,
      service: "phoenix"
    }
    |> Map.merge(sanitize_metadata(metadata))

    new_buffer = [event | state.buffer]

    # Flush if buffer is full
    if length(new_buffer) >= @batch_size do
      flush_buffer(%{state | buffer: new_buffer})
    else
      {:noreply, %{state | buffer: new_buffer}}
    end
  end

  @impl GenServer
  def handle_info(:flush, %{enabled: false} = state) do
    {:noreply, state}
  end

  def handle_info(:flush, state) do
    schedule_flush()
    flush_buffer(state)
  end

  # ----- Private Functions -----

  defp config do
    Application.get_env(:poddyclip_backend, :openobserve)
  end

  defp build_http_client(config) do
    # Build the URL for the OpenObserve API
    base_url = config[:url] || "http://localhost:5080"
    org = config[:org] || "default"
    stream = config[:stream] || "phoenix"

    %{
      url: "#{base_url}/api/#{org}/#{stream}/_json",
      auth: "#{config[:user]}:#{config[:password]}"
    }
  end

  defp schedule_flush do
    Process.send_after(self(), :flush, @flush_interval_ms)
  end

  defp flush_buffer(%{buffer: []} = state) do
    {:noreply, state}
  end

  defp flush_buffer(state) do
    events = Enum.reverse(state.buffer)
    http_client = state.http_client

    # Send synchronously - the 5s flush interval means brief blocking is fine
    send_to_openobserve(events, http_client)

    {:noreply, %{state | buffer: []}}
  end

  defp send_to_openobserve(events, http_client) do
    case Req.post(http_client.url,
           json: events,
           auth: {:basic, http_client.auth},
           receive_timeout: 15_000
         ) do
      {:ok, %{status: status}} when status in 200..299 ->
        :ok

      {:ok, %{status: status, body: body}} ->
        IO.puts("[LogShipper] Failed to ship logs: HTTP #{status} - #{inspect(body)}")

      {:error, reason} ->
        IO.puts("[LogShipper] Failed to ship logs: #{inspect(reason)}")
    end
  end

  defp sanitize_metadata(metadata) when is_map(metadata) do
    metadata
    |> Enum.filter(fn {_k, v} -> is_loggable?(v) end)
    |> Enum.map(fn {k, v} -> {to_string(k), sanitize_field(k, v)} end)
    |> Map.new()
  end

  defp sanitize_metadata(metadata) when is_list(metadata) do
    metadata
    |> Enum.filter(fn {_k, v} -> is_loggable?(v) end)
    |> Enum.map(fn {k, v} -> {to_string(k), sanitize_field(k, v)} end)
    |> Map.new()
  end

  defp is_loggable?(v) when is_binary(v), do: true
  defp is_loggable?(v) when is_number(v), do: true
  defp is_loggable?(v) when is_atom(v), do: true
  defp is_loggable?(v) when is_boolean(v), do: true
  defp is_loggable?(_), do: false

  # Redact sensitive fields
  defp sanitize_field(key, value) do
    key_str = to_string(key) |> String.downcase()

    if is_sensitive_key?(key_str) do
      "[REDACTED]"
    else
      sanitize_value(value)
    end
  end

  defp is_sensitive_key?(key) do
    Enum.any?(@sensitive_keys, fn sensitive ->
      String.contains?(key, sensitive)
    end)
  end

  defp sanitize_value(v) when is_atom(v), do: to_string(v)
  defp sanitize_value(v) when is_binary(v), do: redact_urls(v)
  defp sanitize_value(v), do: v

  # Redact presigned URLs and signatures in string values
  defp redact_urls(value) when is_binary(value) do
    cond do
      # Redact AWS presigned URL signatures
      String.contains?(value, "X-Amz-Signature") ->
        Regex.replace(~r/X-Amz-Signature=[^&]+/, value, "X-Amz-Signature=[REDACTED]")
        |> then(fn v -> Regex.replace(~r/X-Amz-Credential=[^&]+/, v, "X-Amz-Credential=[REDACTED]") end)

      # Redact full presigned URLs (keep just the path)
      String.contains?(value, "?X-Amz-Algorithm") ->
        URI.parse(value).path || "[REDACTED URL]"

      true ->
        value
    end
  end
end
