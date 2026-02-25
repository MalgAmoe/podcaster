defmodule PoddyclipBackendWeb.UserChannel do
  use Phoenix.Channel

  alias PoddyclipBackend.Billing

  @impl true
  def join("user:navbar", _params, socket) do
    user_id = socket.assigns.current_user_id
    Billing.subscribe(user_id)

    # Send current seconds immediately
    user = PoddyclipBackend.Accounts.get_user!(user_id)
    total = Billing.get_total_seconds_available(user)

    {:ok, %{mins: div(total, 60), secs: rem(total, 60)}, socket}
  end

  @impl true
  def handle_info({:user_updated, user}, socket) do
    total = Billing.get_total_seconds_available(user)
    push(socket, "seconds_updated", %{mins: div(total, 60), secs: rem(total, 60)})
    {:noreply, socket}
  end
end
