# Script for populating the database. You can run it as:
#
#     mix run priv/repo/seeds.exs
#
# Inside the script, you can read and write to any of your
# repositories directly:
#
#     PoddyclipBackend.Repo.insert!(%PoddyclipBackend.SomeSchema{})
#
# We recommend using the bang functions (`insert!`, `update!`
# and so on) as they will fail if something goes wrong.

alias PoddyclipBackend.Repo
alias PoddyclipBackend.Billing.Plan

# Create default plans (idempotent - uses ON CONFLICT)
plans = [
  %{
    name: "free",
    display_name: "Free",
    seconds: 1800,
    price_cents: 0,
    polar_product_id: nil,
    active: true
  },
  %{
    name: "munch",
    display_name: "Munch Plan",
    seconds: 18000,
    price_cents: 1500,
    polar_product_id: System.get_env("POLAR_PRO_PRODUCT_ID"),
    active: true
  }
]

for plan_attrs <- plans do
  case Repo.get_by(Plan, name: plan_attrs.name) do
    nil ->
      %Plan{}
      |> Plan.changeset(plan_attrs)
      |> Repo.insert!()
      IO.puts("Created plan: #{plan_attrs.name}")

    existing ->
      existing
      |> Plan.changeset(plan_attrs)
      |> Repo.update!()
      IO.puts("Updated plan: #{plan_attrs.name}")
  end
end
