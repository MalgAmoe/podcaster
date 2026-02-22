defmodule PoddyclipBackend.Billing.Promo do
  @moduledoc """
  Schema for promotional offers.

  Promos give bonus seconds to new signups during a limited time window
  with a limited number of claims. Once created, they work automatically —
  registration checks for an active promo and applies the bonus.

  ## Example

      INSERT INTO promos (name, bonus_seconds, max_claims, starts_at, expires_at, active, inserted_at, updated_at)
      VALUES ('launch-20', 1800, 20, NOW(), NOW() + INTERVAL '24 hours', true, NOW(), NOW());
  """
  use Ecto.Schema
  import Ecto.Changeset

  schema "promos" do
    field :name, :string
    field :bonus_seconds, :integer
    field :max_claims, :integer
    field :claims_count, :integer, default: 0
    field :starts_at, :utc_datetime
    field :expires_at, :utc_datetime
    field :active, :boolean, default: true

    timestamps(type: :utc_datetime)
  end

  @doc false
  def changeset(promo, attrs) do
    promo
    |> cast(attrs, [:name, :bonus_seconds, :max_claims, :claims_count, :starts_at, :expires_at, :active])
    |> validate_required([:name, :bonus_seconds, :max_claims, :starts_at, :expires_at])
    |> validate_number(:bonus_seconds, greater_than: 0)
    |> validate_number(:max_claims, greater_than: 0)
    |> unique_constraint(:name)
  end
end
