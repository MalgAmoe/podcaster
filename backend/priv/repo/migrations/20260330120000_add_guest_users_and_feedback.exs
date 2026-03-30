defmodule PoddyclipBackend.Repo.Migrations.AddGuestUsersAndFeedback do
  use Ecto.Migration

  def change do
    alter table(:users) do
      add :is_guest, :boolean, default: false
      add :completed_jobs_count, :integer, default: 0
    end

    create table(:feedback) do
      add :job_id, references(:jobs, on_delete: :delete_all)
      add :user_id, references(:users, on_delete: :delete_all), null: false
      add :rating, :string
      add :prompt_key, :string
      add :value, :text

      timestamps(type: :utc_datetime, updated_at: false)
    end

    create index(:feedback, [:user_id])
    create index(:feedback, [:job_id])
  end
end
