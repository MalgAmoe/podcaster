defmodule PoddyclipBackend.Repo.Migrations.AddDismissedToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :dismissed, :boolean, default: false, null: false
    end

    create index(:jobs, [:user_id, :dismissed])
  end
end
