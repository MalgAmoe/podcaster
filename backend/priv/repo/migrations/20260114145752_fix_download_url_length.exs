defmodule PoddyclipBackend.Repo.Migrations.FixDownloadUrlLength do
  use Ecto.Migration

  def change do
    alter table(:jobs) do
      modify :download_url, :text, from: :string
    end
  end
end
