defmodule PoddyclipBackendWeb.AccountController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.{Billing, Polar}

  def show(conn, _params) do
    user = conn.assigns.current_scope.user
    user = PoddyclipBackend.Repo.preload(user, :plan)

    # Get plan info
    plan = user.plan || Billing.get_or_create_free_plan()
    is_pro = plan.name == "pro"

    # Calculate usage percentage
    max_minutes = plan.minutes
    used_minutes = max(0, max_minutes - user.minutes_available)
    usage_percent = if max_minutes > 0, do: round(used_minutes / max_minutes * 100), else: 0

    # Generate URLs for upgrade/manage
    pro_plan = Billing.get_plan_by_name("pro")
    checkout_url = if pro_plan, do: Polar.checkout_url(user, pro_plan), else: nil
    portal_url = Polar.customer_portal_url(user)

    # Check for upgrade success
    upgraded = conn.params["upgraded"] == "true"

    render(conn, :show,
      user: user,
      plan: plan,
      is_pro: is_pro,
      max_minutes: max_minutes,
      used_minutes: used_minutes,
      usage_percent: usage_percent,
      checkout_url: checkout_url,
      portal_url: portal_url,
      upgraded: upgraded
    )
  end
end
