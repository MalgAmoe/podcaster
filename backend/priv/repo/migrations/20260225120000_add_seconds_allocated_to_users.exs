defmodule PoddyclipBackend.Repo.Migrations.AddSecondsAllocatedToUsers do
  use Ecto.Migration

  def change do
    alter table(:users) do
      add :seconds_allocated, :integer, default: 900, null: false
    end

    # Backfill: set seconds_allocated to match current seconds_available
    # (best approximation for existing users)
    execute(
      "UPDATE users SET seconds_allocated = GREATEST(seconds_available, (SELECT seconds FROM plans WHERE plans.id = users.plan_id))",
      "SELECT 1"
    )
  end
end
