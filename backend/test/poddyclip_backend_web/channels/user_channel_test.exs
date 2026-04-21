defmodule PoddyclipBackendWeb.UserChannelTest do
  use PoddyclipBackend.DataCase, async: true

  import Phoenix.ChannelTest
  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Repo
  alias PoddyclipBackendWeb.{Endpoint, UserSocket}

  @endpoint Endpoint

  setup do
    free_plan_fixture()
    munch_plan_fixture()

    user =
      user_fixture(%{
        seconds_available: 1800,
        seconds_allocated: 1800
      })
      |> Repo.preload(:plan)

    token = Phoenix.Token.sign(Endpoint, "user socket", user.id)

    {:ok, socket} = connect(UserSocket, %{"token" => token})
    {:ok, join_payload, socket} = subscribe_and_join(socket, PoddyclipBackendWeb.UserChannel, "user:navbar")

    %{socket: socket, user: user, join_payload: join_payload}
  end

  test "join returns current navbar payload", %{join_payload: join_payload} do
    assert join_payload == %{
      mins: 30,
      secs: 0,
      minutes_available: 30,
      plan_display_name: "Free"
    }
  end

  test "pushes updated payload when subscription changes", %{user: user} do
    munch = Billing.get_plan_by_name("munch")

    {:ok, _updated_user} =
      Billing.update_subscription(user, %{
        plan_id: munch.id,
        seconds_available: munch.seconds,
        seconds_allocated: munch.seconds,
        subscription_status: "active"
      })

    assert_push "seconds_updated", %{
      mins: 900,
      secs: 0,
      minutes_available: 900,
      plan_display_name: "Munch Plan"
    }
  end
end
