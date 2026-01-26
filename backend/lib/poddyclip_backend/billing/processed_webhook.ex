defmodule PoddyclipBackend.Billing.ProcessedWebhook do
  @moduledoc """
  Schema for tracking processed webhooks for idempotency.

  Each Polar webhook event has a unique event_id. We track processed events
  to prevent duplicate processing in case of webhook retries.
  """
  use Ecto.Schema
  import Ecto.Changeset

  schema "processed_webhooks" do
    field :event_id, :string
    field :event_type, :string

    timestamps(type: :utc_datetime)
  end

  @doc false
  def changeset(webhook, attrs) do
    webhook
    |> cast(attrs, [:event_id, :event_type])
    |> validate_required([:event_id, :event_type])
    |> unique_constraint(:event_id)
  end
end
