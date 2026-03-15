defmodule PoddyclipBackend.Repo.Migrations.CreateDemoUser do
  use Ecto.Migration

  def up do
    # Create the demo user with high seconds balance
    # The free plan is looked up by name to get the plan_id
    execute """
    INSERT INTO users (email, hashed_password, seconds_available, subscription_status, plan_id, inserted_at, updated_at)
    SELECT 'demo@munchycow.com', 'demo_no_login', 999999, 'none', p.id, NOW(), NOW()
    FROM plans p WHERE p.name = 'free'
    ON CONFLICT (email) DO NOTHING
    """
  end

  def down do
    execute "DELETE FROM users WHERE email = 'demo@munchycow.com'"
  end
end
