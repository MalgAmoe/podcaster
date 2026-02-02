defmodule PoddyclipBackend.Repo.Migrations.RenameMinutesToSeconds do
  use Ecto.Migration

  def up do
    # Rename user minutes_available to seconds_available and convert values
    rename table(:users), :minutes_available, to: :seconds_available
    execute "UPDATE users SET seconds_available = seconds_available * 60"

    # Rename job estimated_minutes to estimated_seconds and convert values
    rename table(:jobs), :estimated_minutes, to: :estimated_seconds
    execute "UPDATE jobs SET estimated_seconds = estimated_seconds * 60 WHERE estimated_seconds IS NOT NULL"

    # Rename plan minutes to seconds and convert values
    rename table(:plans), :minutes, to: :seconds
    execute "UPDATE plans SET seconds = seconds * 60"
  end

  def down do
    # Convert back to minutes (integer division)
    execute "UPDATE users SET seconds_available = seconds_available / 60"
    rename table(:users), :seconds_available, to: :minutes_available

    execute "UPDATE jobs SET estimated_seconds = estimated_seconds / 60 WHERE estimated_seconds IS NOT NULL"
    rename table(:jobs), :estimated_seconds, to: :estimated_minutes

    execute "UPDATE plans SET seconds = seconds / 60"
    rename table(:plans), :seconds, to: :minutes
  end
end
