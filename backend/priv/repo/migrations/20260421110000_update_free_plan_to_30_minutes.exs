defmodule PoddyclipBackend.Repo.Migrations.UpdateFreePlanTo30Minutes do
  use Ecto.Migration

  def up do
    execute "UPDATE plans SET seconds = 1800 WHERE name = 'free'"

    execute """
    UPDATE users
    SET
      seconds_available = LEAST(seconds_available, 1800),
      seconds_allocated = 1800
    WHERE plan_id IN (SELECT id FROM plans WHERE name = 'free')
      AND subscription_status = 'none'
    """
  end

  def down do
    execute "UPDATE plans SET seconds = 10800 WHERE name = 'free'"

    execute """
    UPDATE users
    SET
      seconds_available = 10800,
      seconds_allocated = 10800
    WHERE plan_id IN (SELECT id FROM plans WHERE name = 'free')
      AND subscription_status = 'none'
      AND seconds_available <= 1800
      AND seconds_allocated = 1800
    """
  end
end
