defmodule PoddyclipBackend.Repo.Migrations.RenameLowMinutesToLowTime do
  use Ecto.Migration

  def up do
    # Rename the database column
    rename table(:users), :last_low_minutes_notification_at, to: :last_low_time_notification_at

    # Update the JSON key in notification_preferences
    execute """
    UPDATE users
    SET notification_preferences = jsonb_set(
      notification_preferences - 'low_minutes',
      '{low_time}',
      COALESCE(notification_preferences->'low_minutes', 'true'::jsonb)
    )
    WHERE notification_preferences ? 'low_minutes'
    """
  end

  def down do
    rename table(:users), :last_low_time_notification_at, to: :last_low_minutes_notification_at

    execute """
    UPDATE users
    SET notification_preferences = jsonb_set(
      notification_preferences - 'low_time',
      '{low_minutes}',
      COALESCE(notification_preferences->'low_time', 'true'::jsonb)
    )
    WHERE notification_preferences ? 'low_time'
    """
  end
end
