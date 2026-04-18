defmodule PoddyclipBackend.Processing.ClientTest do
  use ExUnit.Case, async: false

  alias PoddyclipBackend.Processing.Client

  setup do
    original_url = Application.get_env(:poddyclip_backend, :poddyclip_api_url)
    port = System.unique_integer([:positive]) + 40_000
    base_url = "http://127.0.0.1:#{port}"

    Application.put_env(:poddyclip_backend, :poddyclip_api_url, base_url)

    on_exit(fn ->
      if original_url do
        Application.put_env(:poddyclip_backend, :poddyclip_api_url, original_url)
      else
        Application.delete_env(:poddyclip_backend, :poddyclip_api_url)
      end
    end)

    %{port: port}
  end

  test "preview returns wav bytes on success", %{port: port} do
    {:ok, _pid} =
      start_preview_server(port, fn conn ->
        conn
        |> Plug.Conn.put_resp_content_type("audio/wav")
        |> Plug.Conn.send_resp(200, "processed-wav")
      end)

    assert Client.preview("wav-bytes") == {:ok, "processed-wav"}
  end

  test "preview maps too_long responses", %{port: port} do
    {:ok, _pid} =
      start_preview_server(port, fn conn ->
        body = ~s({"error":{"type":"too_long"},"max_seconds":30,"tolerance_seconds":1})

        conn
        |> Plug.Conn.put_resp_content_type("application/json")
        |> Plug.Conn.send_resp(422, body)
      end)

    assert Client.preview("wav-bytes") == {:error, {:too_long, 30, 1}}
  end

  test "preview maps rust busy without inventing a retry time", %{port: port} do
    {:ok, _pid} =
      start_preview_server(port, fn conn ->
        conn
        |> Plug.Conn.put_resp_content_type("application/json")
        |> Plug.Conn.send_resp(503, ~s({"error":{"type":"preview_busy"}}))
      end)

    assert Client.preview("wav-bytes") == {:error, :preview_busy}
  end

  defp start_preview_server(port, handler) do
    plug = {__MODULE__.PreviewStubPlug, handler}

    Bandit.start_link(
      plug: plug,
      scheme: :http,
      ip: {127, 0, 0, 1},
      port: port,
      startup_log: false
    )
  end

  defmodule PreviewStubPlug do
    def init(handler), do: handler

    def call(conn, handler) do
      handler.(conn)
    end
  end
end
