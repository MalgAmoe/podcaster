defmodule PoddyclipBackend.Billing.BillingEvent do
  @moduledoc """
  Audit log for automated billing actions (worker resets, downgrades, notifications).
  """
  use Ecto.Schema
  import Ecto.Changeset

  schema "billing_events" do
    field :event_type, :string
    field :metadata, :map, default: %{}

    belongs_to :user, PoddyclipBackend.Accounts.User

    timestamps(type: :utc_datetime, updated_at: false)
  end

  def changeset(billing_event, attrs) do
    billing_event
    |> cast(attrs, [:event_type, :user_id, :metadata])
    |> validate_required([:event_type, :user_id])
    |> validate_inclusion(:event_type, [
      "free_plan_reset",
      "subscription_downgraded",
      "expiry_notification_sent"
    ])
    |> foreign_key_constraint(:user_id)
  end
end
