defmodule PoddyclipBackend.Repo.Migrations.AddInputS3KeyToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :input_s3_key, :string
      add :chain, :string
    end
  end
end
