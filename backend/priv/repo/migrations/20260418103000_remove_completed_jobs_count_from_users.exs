defmodule PoddyclipBackend.Repo.Migrations.RemoveCompletedJobsCountFromUsers do
  use Ecto.Migration

  def change do
    alter table(:users) do
      remove :completed_jobs_count, :integer
    end
  end
end
