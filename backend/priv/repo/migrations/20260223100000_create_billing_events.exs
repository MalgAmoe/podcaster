defmodule PoddyclipBackend.Repo.Migrations.CreateBillingEvents do
  use Ecto.Migration

  def change do
    create table(:billing_events) do
      add :event_type, :string, null: false
      add :user_id, references(:users, on_delete: :delete_all), null: false
      add :metadata, :map, default: %{}

      timestamps(type: :utc_datetime, updated_at: false)
    end

    create index(:billing_events, [:event_type, :inserted_at])
    create index(:billing_events, [:user_id])
  end
end
