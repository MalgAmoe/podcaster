defmodule PoddyclipBackendWeb.AccountLive do
  use PoddyclipBackendWeb, :live_view

  alias PoddyclipBackend.{Billing, Polar, Repo}
  alias PoddyclipBackend.Billing.MinutePack
  alias PoddyclipBackendWeb.Endpoint

  @impl true
  def mount(_params, _session, socket) do
    user = socket.assigns.current_scope.user |> Repo.preload(:plan)

    if connected?(socket) do
      Billing.subscribe(user.id)
    end

    {:ok, assign_user_data(socket, user)}
  end

  @impl true
  def handle_params(params, _uri, socket) do
    upgraded = params["upgraded"] == "true"
    snack_purchased = params["snack_purchased"] == "true"

    # Track initial pack count to detect when webhook arrives
    socket =
      if snack_purchased and not Map.has_key?(socket.assigns, :initial_pack_count) do
        assign(socket, initial_pack_count: socket.assigns.pack_count)
      else
        socket
      end

    {:noreply, assign(socket, upgraded: upgraded, snack_purchased: snack_purchased)}
  end

  @impl true
  def handle_info({:user_updated, user}, socket) do
    user = Repo.preload(user, :plan)
    {:noreply, assign_user_data(socket, user)}
  end

  defp assign_user_data(socket, user) do
    plan = user.plan || Billing.get_or_create_free_plan()
    is_pro = plan.name == "munch"
    max_seconds = user.seconds_allocated
    used_seconds = max(0, max_seconds - user.seconds_available)
    usage_percent = if max_seconds > 0, do: round(used_seconds / max_seconds * 100), else: 0
    # Convert to minutes and seconds for display
    max_minutes = div(max_seconds, 60)
    used_min = div(used_seconds, 60)
    used_sec = rem(used_seconds, 60)
    remaining_min = div(user.seconds_available, 60)
    remaining_sec = rem(user.seconds_available, 60)

    # Get minute pack summary
    pack_summary = Billing.get_pack_summary(user.id)
    pack_minutes = div(pack_summary.total_seconds, 60)
    pack_seconds = rem(pack_summary.total_seconds, 60)

    # Check if snack purchase is pending (waiting for webhook)
    initial_pack_count = socket.assigns[:initial_pack_count]
    snack_pending = initial_pack_count != nil and pack_summary.pack_count <= initial_pack_count

    assign(socket,
      user: user,
      plan: plan,
      is_pro: is_pro,
      max_seconds: max_seconds,
      max_minutes: max_minutes,
      used_min: used_min,
      used_sec: used_sec,
      remaining_min: remaining_min,
      remaining_sec: remaining_sec,
      usage_percent: usage_percent,
      checkout_url: Polar.checkout_url(user, Billing.get_plan_by_name("munch")),
      portal_url: Polar.customer_portal_url(user),
      # Minute packs
      pack_minutes: pack_minutes,
      pack_seconds: pack_seconds,
      pack_count: pack_summary.pack_count,
      snack_pending: snack_pending,
      minute_pack_checkout_url: Polar.minute_pack_checkout_url(user, Endpoint.url() <> "/account?snack_purchased=true"),
      minute_pack_price: "$#{MinutePack.pack_price_cents() / 100}",
      minute_pack_minutes: div(MinutePack.pack_seconds(), 60)
    )
  end

  # Helper for template to check if upgrade is pending
  def upgrade_pending?(assigns) do
    assigns.upgraded and not assigns.is_pro
  end
end
