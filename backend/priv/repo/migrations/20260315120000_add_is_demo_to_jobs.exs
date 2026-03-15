defmodule PoddyclipBackend.Repo.Migrations.AddIsDemoToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :is_demo, :boolean, default: false
    end
  end
end
