defmodule PoddyclipBackend.Repo.Migrations.CreateJobs do
  use Ecto.Migration

  def change do
    create table(:jobs) do
      add :rust_job_id, :string
      add :filename, :string, null: false
      add :status, :string, null: false, default: "queued"
      add :progress, :map, default: %{}
      add :error, :text
      add :download_url, :string
      add :user_id, references(:users, on_delete: :delete_all), null: false

      timestamps(type: :utc_datetime)
    end

    create index(:jobs, [:user_id])
    create index(:jobs, [:status])
    create index(:jobs, [:rust_job_id])
  end
end
