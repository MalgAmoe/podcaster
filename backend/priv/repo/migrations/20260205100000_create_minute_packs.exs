defmodule PoddyclipBackend.Repo.Migrations.CreateMinutePacks do
  use Ecto.Migration

  def change do
    create table(:minute_packs) do
      add :user_id, references(:users, on_delete: :delete_all), null: false
      add :seconds_total, :integer, null: false
      add :seconds_remaining, :integer, null: false
      add :price_cents, :integer, null: false
      add :polar_order_id, :string
      add :purchased_at, :utc_datetime, null: false
      add :expires_at, :utc_datetime, null: false

      timestamps(type: :utc_datetime)
    end

    create index(:minute_packs, [:user_id])
    create index(:minute_packs, [:expires_at])
    create unique_index(:minute_packs, [:polar_order_id], where: "polar_order_id IS NOT NULL")
  end
end
