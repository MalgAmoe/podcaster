defmodule PoddyclipBackend.Repo.Migrations.SetFreePlanPeriod do
  use Ecto.Migration

  def up do
    # For existing free users: set current_period_ends_at
    # - If inserted_at is within last 30 days: set to inserted_at + 30 days
    # - If inserted_at is older than 30 days: set to now (so next worker run resets them)
    execute """
    UPDATE users
    SET current_period_ends_at = CASE
      WHEN inserted_at > (NOW() - INTERVAL '30 days')
        THEN inserted_at + INTERVAL '30 days'
      ELSE NOW()
    END
    WHERE subscription_status = 'none'
      AND current_period_ends_at IS NULL
    """
  end

  def down do
    execute """
    UPDATE users
    SET current_period_ends_at = NULL
    WHERE subscription_status = 'none'
    """
  end
end
