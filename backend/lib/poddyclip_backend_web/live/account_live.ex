defmodule PoddyclipBackendWeb.AccountLive do
  use PoddyclipBackendWeb, :live_view

  alias PoddyclipBackend.{Billing, Polar, Repo}

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
    {:noreply, assign(socket, upgraded: upgraded)}
  end

  @impl true
  def handle_info({:user_updated, user}, socket) do
    user = Repo.preload(user, :plan)
    {:noreply, assign_user_data(socket, user)}
  end

  defp assign_user_data(socket, user) do
    plan = user.plan || Billing.get_or_create_free_plan()
    is_pro = plan.name == "pro"
    max_minutes = plan.minutes
    used_minutes = max(0, max_minutes - user.minutes_available)
    usage_percent = if max_minutes > 0, do: round(used_minutes / max_minutes * 100), else: 0

    assign(socket,
      user: user,
      plan: plan,
      is_pro: is_pro,
      max_minutes: max_minutes,
      used_minutes: used_minutes,
      usage_percent: usage_percent,
      checkout_url: Polar.checkout_url(user, Billing.get_plan_by_name("pro")),
      portal_url: Polar.customer_portal_url(user)
    )
  end

  # Helper for template to check if upgrade is pending
  def upgrade_pending?(assigns) do
    assigns.upgraded and not assigns.is_pro
  end
end
