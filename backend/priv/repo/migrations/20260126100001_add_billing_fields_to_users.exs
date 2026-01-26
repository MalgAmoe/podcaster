defmodule PoddyclipBackend.Repo.Migrations.AddBillingFieldsToUsers do
  use Ecto.Migration

  def change do
    alter table(:users) do
      add :plan_id, references(:plans, on_delete: :nothing)
      add :minutes_available, :integer, default: 15, null: false
      add :subscription_status, :string, default: "none", null: false
      add :polar_customer_id, :string
      add :polar_subscription_id, :string
      add :current_period_ends_at, :utc_datetime
    end

    create index(:users, [:polar_customer_id])
    create index(:users, [:polar_subscription_id])
    create index(:users, [:plan_id])
  end
end
