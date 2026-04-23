defmodule PoddyclipBackendWeb.PreviewChannel do
  @moduledoc """
  Transient guest preview channel `preview:{request_id}` pushing queued and
  processing state updates while the preview request is still in flight.
  """

  use PoddyclipBackendWeb, :channel

  alias PoddyclipBackend.PreviewGate

  @impl true
  def join("preview:" <> request_id, _params, socket) do
    user_id = socket.assigns.current_user_id

    case PreviewGate.get_preview_status(user_id, request_id) do
      {:ok, payload} ->
        Phoenix.PubSub.subscribe(PoddyclipBackend.PubSub, "preview:#{request_id}")
        {:ok, payload, socket}

      {:error, :unauthorized} ->
        {:error, %{reason: "unauthorized"}}
    end
  end

  @impl true
  def handle_info({:preview_updated, payload}, socket) do
    push(socket, "preview_updated", payload)
    {:noreply, socket}
  end
end
