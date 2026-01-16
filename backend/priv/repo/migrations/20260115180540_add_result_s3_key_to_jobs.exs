defmodule PoddyclipBackend.Repo.Migrations.AddResultS3KeyToJobs do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      add :result_s3_key, :string
    end
  end
end
