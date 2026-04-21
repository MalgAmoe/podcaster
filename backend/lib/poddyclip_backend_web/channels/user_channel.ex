defmodule PoddyclipBackendWeb.UserChannel do
  @moduledoc """
  `user:navbar` channel pushing live updates to the user's remaining
  processing seconds.
  """
  use Phoenix.Channel

  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Repo

  @impl true
  def join("user:navbar", _params, socket) do
    user_id = socket.assigns.current_user_id
    Billing.subscribe(user_id)

    # Send current seconds immediately
    user = PoddyclipBackend.Accounts.get_user!(user_id) |> Repo.preload(:plan)

    {:ok, navbar_payload(user), socket}
  end

  @impl true
  def handle_info({:user_updated, user}, socket) do
    push(socket, "seconds_updated", navbar_payload(user |> Repo.preload(:plan)))
    {:noreply, socket}
  end

  defp navbar_payload(user) do
    total = Billing.get_total_seconds_available(user)
    plan = user.plan || Billing.get_or_create_free_plan()

    %{
      mins: div(total, 60),
      secs: rem(total, 60),
      minutes_available: div(total, 60),
      plan_display_name: plan.display_name
    }
  end
end
