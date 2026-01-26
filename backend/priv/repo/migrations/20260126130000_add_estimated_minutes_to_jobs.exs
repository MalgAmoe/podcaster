defmodule PoddyclipBackend.Repo.Migrations.AddEstimatedMinutesToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :estimated_minutes, :integer
    end
  end
end
