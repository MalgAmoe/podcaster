defmodule PoddyclipBackendWeb.PreviewChannelTest do
  use PoddyclipBackend.DataCase, async: true

  import Phoenix.ChannelTest

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.PreviewGate
  alias PoddyclipBackendWeb.{Endpoint, UserSocket}

  @endpoint Endpoint

  setup do
    {:ok, guest} = Accounts.create_guest_user()
    token = Phoenix.Token.sign(Endpoint, "user socket", guest.id)
    {:ok, socket} = connect(UserSocket, %{"token" => token})

    on_exit(fn ->
      PreviewGate.reset_for_test()
    end)

    %{guest: guest, socket: socket}
  end

  test "join returns current preview status and receives updates", %{guest: guest, socket: socket} do
    request_id = "preview-test-1"

    :ok = PreviewGate.register_preview_request(guest.id, request_id)

    {:ok, join_payload, _socket} =
      subscribe_and_join(socket, PoddyclipBackendWeb.PreviewChannel, "preview:#{request_id}")

    assert join_payload == %{request_id: request_id, status: "queued"}

    :ok = PreviewGate.mark_preview_starting(guest.id, request_id)

    assert_push "preview_updated", %{request_id: ^request_id, status: "starting"}

    :ok = PreviewGate.mark_preview_processing(guest.id, request_id)
    assert_push "preview_updated", %{request_id: ^request_id, status: "processing"}
  end
end
