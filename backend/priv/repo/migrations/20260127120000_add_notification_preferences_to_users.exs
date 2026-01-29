defmodule PoddyclipBackend.Repo.Migrations.AddNotificationPreferencesToUsers do
  use Ecto.Migration

  def change do
    alter table(:users) do
      add :notification_preferences, :map, default: %{
        "job_complete" => true,
        "job_failed" => true,
        "low_minutes" => true,
        "subscription_expiry" => true
      }
      add :last_low_minutes_notification_at, :utc_datetime
      add :expiry_notification_sent_at, :utc_datetime
    end
  end
end
