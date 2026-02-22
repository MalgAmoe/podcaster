defmodule PoddyclipBackend.Repo.Migrations.CreatePromos do
  use Ecto.Migration

  def change do
    create table(:promos) do
      add :name, :string, null: false
      add :bonus_seconds, :integer, null: false
      add :max_claims, :integer, null: false
      add :claims_count, :integer, null: false, default: 0
      add :starts_at, :utc_datetime, null: false
      add :expires_at, :utc_datetime, null: false
      add :active, :boolean, null: false, default: true

      timestamps(type: :utc_datetime)
    end

    create unique_index(:promos, [:name])
  end
end
