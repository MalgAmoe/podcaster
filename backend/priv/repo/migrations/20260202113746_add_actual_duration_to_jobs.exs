defmodule PoddyclipBackend.Repo.Migrations.AddActualDurationToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :actual_duration_seconds, :integer
    end
  end
end
