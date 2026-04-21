defmodule PoddyclipBackend.Billing.Plan do
  @moduledoc """
  Schema for subscription plans.

  Plans define the available subscription tiers:
  - free: 1800 seconds (30 minutes), no payment required
  - pro: 54000 seconds (15 hours), $15/month
  """
  use Ecto.Schema
  import Ecto.Changeset

  schema "plans" do
    field :name, :string
    field :display_name, :string
    field :seconds, :integer
    field :price_cents, :integer
    field :polar_product_id, :string
    field :active, :boolean, default: true

    timestamps(type: :utc_datetime)
  end

  @doc false
  def changeset(plan, attrs) do
    plan
    |> cast(attrs, [:name, :display_name, :seconds, :price_cents, :polar_product_id, :active])
    |> validate_required([:name, :display_name, :seconds, :price_cents])
    |> unique_constraint(:name)
  end
end
