defmodule PoddyclipBackendWeb.ProcessControllerTest do
  use PoddyclipBackendWeb.ConnCase, async: false

  import PoddyclipBackend.AccountsFixtures

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.PreviewGate

  @moduletag :tmp_dir

  setup %{tmp_dir: tmp_dir} do
    {:ok, guest} = Accounts.create_guest_user()
    conn = log_in_user(Phoenix.ConnTest.build_conn(), guest)
    audio_path = Path.join(tmp_dir, "preview.wav")
    File.write!(audio_path, "fake preview bytes")

    previous_env = %{
      poddyclip_api_url: Application.get_env(:poddyclip_backend, :poddyclip_api_url),
      preview_burst_limit: Application.get_env(:poddyclip_backend, :preview_burst_limit),
      preview_rate_window_ms: Application.get_env(:poddyclip_backend, :preview_rate_window_ms),
      preview_busy_retry_ms: Application.get_env(:poddyclip_backend, :preview_busy_retry_ms),
      preview_max_inflight: Application.get_env(:poddyclip_backend, :preview_max_inflight)
    }

    on_exit(fn ->
      restore_env(previous_env)
      :ets.delete_all_objects(:preview_gate_timestamps)
      :sys.replace_state(PreviewGate, fn _ -> %{active_tokens: %{}} end)
    end)

    {:ok, conn: conn, audio_path: audio_path, guest: guest}
  end

  test "POST /api/preview returns wav bytes on success", %{conn: conn, audio_path: audio_path} do
    start_preview_server(fn conn ->
      Plug.Conn.send_resp(conn, 200, "processed-wav")
    end)

    conn =
      post(conn, ~p"/api/preview", %{
        "audio" => %Plug.Upload{
          path: audio_path,
          filename: "preview.wav",
          content_type: "audio/wav"
        }
      })

    assert response(conn, 200) == "processed-wav"
    assert get_resp_header(conn, "content-type") == ["audio/wav; charset=utf-8"]
  end

  test "POST /api/preview maps too_long errors", %{conn: conn, audio_path: audio_path} do
    start_preview_server(fn conn ->
      body = ~s({"error":{"type":"too_long","message":"too long"},"max_seconds":30,"tolerance_seconds":2.0})

      conn
      |> Plug.Conn.put_resp_content_type("application/json")
      |> Plug.Conn.send_resp(422, body)
    end)

    conn =
      post(conn, ~p"/api/preview", %{
        "audio" => %Plug.Upload{
          path: audio_path,
          filename: "preview.wav",
          content_type: "audio/wav"
        }
      })

    assert json_response(conn, 422) == %{
             "error" => "too_long",
             "max_seconds" => 30,
             "tolerance_seconds" => 2.0
           }
  end

  test "POST /api/preview maps rate-limited guests", %{conn: conn, audio_path: audio_path} do
    Application.put_env(:poddyclip_backend, :preview_burst_limit, 3)
    Application.put_env(:poddyclip_backend, :preview_rate_window_ms, 900_000)

    start_preview_server(fn conn ->
      Plug.Conn.send_resp(conn, 200, "processed-wav")
    end)

    upload = %Plug.Upload{
      path: audio_path,
      filename: "preview.wav",
      content_type: "audio/wav"
    }

    user_conn = get(conn, ~p"/api/user")
    current_user = json_response(user_conn, 200)

    now_ms = System.system_time(:millisecond)
    :ets.insert(:preview_gate_timestamps, {current_user["id"], now_ms - 3_000})
    :ets.insert(:preview_gate_timestamps, {current_user["id"], now_ms - 2_000})
    :ets.insert(:preview_gate_timestamps, {current_user["id"], now_ms - 1_000})

    conn = post(recycle(user_conn), ~p"/api/preview", %{"audio" => upload})
    body = json_response(conn, 429)

    assert body["error"] == "rate_limited"
    assert is_integer(body["retry_after_ms"])
    assert body["retry_after_ms"] > 0
  end

  test "POST /api/preview returns preview_busy when demo slot is occupied", %{
    conn: conn,
    audio_path: audio_path
  } do
    Application.put_env(:poddyclip_backend, :preview_max_inflight, 1)
    Application.put_env(:poddyclip_backend, :preview_busy_retry_ms, 10_000)

    parent = self()

    start_preview_server(fn conn ->
      send(parent, :preview_request_started)
      Process.sleep(300)
      Plug.Conn.send_resp(conn, 200, "processed-wav")
    end)

    upload = %Plug.Upload{
      path: audio_path,
      filename: "preview.wav",
      content_type: "audio/wav"
    }

    task =
      Task.async(fn ->
        post(recycle(conn), ~p"/api/preview", %{"audio" => upload})
      end)

    assert_receive :preview_request_started, 1_000

    busy_conn = post(recycle(conn), ~p"/api/preview", %{"audio" => upload})
    body = json_response(busy_conn, 503)

    assert body["error"] == "preview_busy"
    assert body["retry_after_ms"] == 10_000

    _ = Task.await(task, 2_000)
  end

  test "POST /api/preview does not rate limit signed-in users", %{audio_path: audio_path} do
    user = user_fixture()
    conn = log_in_user(Phoenix.ConnTest.build_conn(), user)

    Application.put_env(:poddyclip_backend, :preview_burst_limit, 3)
    Application.put_env(:poddyclip_backend, :preview_rate_window_ms, 900_000)

    start_preview_server(fn conn ->
      Plug.Conn.send_resp(conn, 200, "processed-wav")
    end)

    upload = %Plug.Upload{
      path: audio_path,
      filename: "preview.wav",
      content_type: "audio/wav"
    }

    for _ <- 1..4 do
      conn = post(recycle(conn), ~p"/api/preview", %{"audio" => upload})
      assert response(conn, 200) == "processed-wav"
    end
  end

  defmodule PreviewStubPlug do
    def init(opts), do: opts

    def call(conn, opts) do
      opts[:handler].(conn)
    end
  end

  defp start_preview_server(handler) do
    port = reserve_port()
    Application.put_env(:poddyclip_backend, :poddyclip_api_url, "http://127.0.0.1:#{port}")

    start_supervised!(
      {Bandit,
       plug: {PreviewStubPlug, handler: handler},
       scheme: :http,
       port: port}
    )
  end

  defp reserve_port do
    {:ok, socket} = :gen_tcp.listen(0, [:binary, active: false, packet: :raw])
    {:ok, port} = :inet.port(socket)
    :gen_tcp.close(socket)
    port
  end

  defp restore_env(previous_env) do
    Enum.each(previous_env, fn {key, value} ->
      if is_nil(value) do
        Application.delete_env(:poddyclip_backend, key)
      else
        Application.put_env(:poddyclip_backend, key, value)
      end
    end)
  end
end
