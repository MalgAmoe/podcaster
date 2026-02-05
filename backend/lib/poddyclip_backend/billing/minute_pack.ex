defmodule PoddyclipBackend.Billing.MinutePack do
  @moduledoc """
  Schema for purchasable minute packs.

  Minute packs allow users to buy additional processing time:
  - $5 for 150 minutes (9000 seconds)
  - Expire 1 year from purchase
  - Usage order: subscription seconds first, then packs (FIFO by expiry)
  - Both Free and Pro users can purchase packs
  """
  use Ecto.Schema
  import Ecto.Changeset

  @pack_seconds 9000
  @pack_price_cents 500
  @expiry_days 365

  schema "minute_packs" do
    field :seconds_total, :integer
    field :seconds_remaining, :integer
    field :price_cents, :integer
    field :polar_order_id, :string
    field :purchased_at, :utc_datetime
    field :expires_at, :utc_datetime

    belongs_to :user, PoddyclipBackend.Accounts.User

    timestamps(type: :utc_datetime)
  end

  @doc """
  Returns the default pack configuration.
  """
  def pack_seconds, do: @pack_seconds
  def pack_price_cents, do: @pack_price_cents
  def expiry_days, do: @expiry_days

  @doc """
  Creates a changeset for a new minute pack purchase.
  """
  def create_changeset(minute_pack, attrs) do
    minute_pack
    |> cast(attrs, [:user_id, :polar_order_id])
    |> validate_required([:user_id])
    |> put_defaults()
    |> unique_constraint(:polar_order_id)
    |> foreign_key_constraint(:user_id)
  end

  @doc """
  Creates a changeset for deducting seconds from a pack.
  """
  def deduct_changeset(minute_pack, seconds_to_deduct) do
    new_remaining = max(0, minute_pack.seconds_remaining - seconds_to_deduct)

    minute_pack
    |> change(seconds_remaining: new_remaining)
  end

  defp put_defaults(changeset) do
    now = DateTime.utc_now() |> DateTime.truncate(:second)
    expires_at = DateTime.add(now, @expiry_days, :day)

    changeset
    |> put_change(:seconds_total, @pack_seconds)
    |> put_change(:seconds_remaining, @pack_seconds)
    |> put_change(:price_cents, @pack_price_cents)
    |> put_change(:purchased_at, now)
    |> put_change(:expires_at, expires_at)
  end
end
