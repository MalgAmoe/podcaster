defmodule PoddyclipBackend.Repo.Migrations.AddCompoundIndexesToJobs do
  use Ecto.Migration

  @doc """
  Add compound indexes for frequently queried job lookups.

  These indexes optimize:
  - Job history query: WHERE user_id = ? AND status = 'completed' ORDER BY inserted_at
  - Current job query: WHERE user_id = ? AND dismissed = false ORDER BY updated_at
  """
  def change do
    # For job history query: list_completed_jobs_for_user/1
    create index(:jobs, [:user_id, :status, :inserted_at])

    # For current job query: get_current_job/1
    create index(:jobs, [:user_id, :dismissed, :updated_at])
  end
end
