defmodule PoddyclipBackend.Repo.Migrations.UpdateFreePlanTo3Hours do
  use Ecto.Migration

  def up do
    # Update free plan from 900 seconds (15 min) to 10800 seconds (3 hours)
    execute "UPDATE plans SET seconds = 10800 WHERE name = 'free'"

    # Update existing free users who still have the old default (900 seconds)
    # Only update users on the free plan (subscription_status = 'none') who haven't used any seconds
    execute """
    UPDATE users
    SET seconds_available = 10800, seconds_allocated = 10800
    WHERE plan_id = (SELECT id FROM plans WHERE name = 'free')
      AND subscription_status = 'none'
      AND seconds_available = 900
      AND seconds_allocated = 900
    """
  end

  def down do
    execute "UPDATE plans SET seconds = 900 WHERE name = 'free'"

    execute """
    UPDATE users
    SET seconds_available = 900, seconds_allocated = 900
    WHERE plan_id = (SELECT id FROM plans WHERE name = 'free')
      AND subscription_status = 'none'
      AND seconds_available = 10800
      AND seconds_allocated = 10800
    """
  end
end
