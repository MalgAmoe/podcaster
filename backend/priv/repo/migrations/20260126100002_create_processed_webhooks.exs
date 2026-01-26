defmodule PoddyclipBackend.Repo.Migrations.CreateProcessedWebhooks do
  use Ecto.Migration

  def change do
    create table(:processed_webhooks) do
      add :event_id, :string, null: false
      add :event_type, :string, null: false

      timestamps(type: :utc_datetime)
    end

    create unique_index(:processed_webhooks, [:event_id])
  end
end
