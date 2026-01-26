defmodule PoddyclipBackend.BillingFixtures do
  @moduledoc """
  Test helpers for creating billing-related entities.
  """

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Billing.Plan

  def free_plan_fixture(attrs \\ %{}) do
    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: "free",
          display_name: "Free",
          minutes: 15,
          price_cents: 0,
          active: true
        })
      )
      |> Repo.insert(on_conflict: :nothing)

    # Return existing if conflict
    case plan.id do
      nil -> Repo.get_by!(Plan, name: attrs[:name] || "free")
      _ -> plan
    end
  end

  def pro_plan_fixture(attrs \\ %{}) do
    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: "pro",
          display_name: "Pro",
          minutes: 900,
          price_cents: 1500,
          polar_product_id: "prod_test_123",
          active: true
        })
      )
      |> Repo.insert(on_conflict: :nothing)

    case plan.id do
      nil -> Repo.get_by!(Plan, name: attrs[:name] || "pro")
      _ -> plan
    end
  end

  def plan_fixture(attrs \\ %{}) do
    name = attrs[:name] || "test_plan_#{System.unique_integer([:positive])}"

    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: name,
          display_name: String.capitalize(name),
          minutes: 100,
          price_cents: 1000,
          active: true
        })
      )
      |> Repo.insert()

    plan
  end
end
