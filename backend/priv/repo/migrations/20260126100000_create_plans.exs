defmodule PoddyclipBackend.Repo.Migrations.CreatePlans do
  use Ecto.Migration

  def change do
    create table(:plans) do
      add :name, :string, null: false
      add :display_name, :string, null: false
      add :minutes, :integer, null: false
      add :price_cents, :integer, null: false
      add :polar_product_id, :string
      add :active, :boolean, default: true, null: false

      timestamps(type: :utc_datetime)
    end

    create unique_index(:plans, [:name])
  end
end
