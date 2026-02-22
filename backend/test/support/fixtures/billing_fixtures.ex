defmodule PoddyclipBackend.BillingFixtures do
  @moduledoc """
  Test helpers for creating billing-related entities.
  """

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Billing.{Plan, Promo}

  def free_plan_fixture(attrs \\ %{}) do
    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: "free",
          display_name: "Free",
          seconds: 900,
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

  def munch_plan_fixture(attrs \\ %{}) do
    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: "munch",
          display_name: "Munch Plan",
          seconds: 54000,
          price_cents: 1500,
          polar_product_id: "prod_test_123",
          active: true
        })
      )
      |> Repo.insert(on_conflict: :nothing)

    case plan.id do
      nil -> Repo.get_by!(Plan, name: attrs[:name] || "munch")
      _ -> plan
    end
  end

  def promo_fixture(attrs \\ %{}) do
    now = DateTime.utc_now() |> DateTime.truncate(:second)

    {:ok, promo} =
      %Promo{}
      |> Promo.changeset(
        Enum.into(attrs, %{
          name: "test-promo-#{System.unique_integer([:positive])}",
          bonus_seconds: 1800,
          max_claims: 20,
          starts_at: DateTime.add(now, -1, :hour),
          expires_at: DateTime.add(now, 24, :hour),
          active: true
        })
      )
      |> Repo.insert()

    promo
  end

  def plan_fixture(attrs \\ %{}) do
    name = attrs[:name] || "test_plan_#{System.unique_integer([:positive])}"

    {:ok, plan} =
      %Plan{}
      |> Plan.changeset(
        Enum.into(attrs, %{
          name: name,
          display_name: String.capitalize(name),
          seconds: 6000,
          price_cents: 1000,
          active: true
        })
      )
      |> Repo.insert()

    plan
  end
end
