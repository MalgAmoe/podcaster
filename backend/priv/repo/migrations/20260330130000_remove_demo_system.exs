defmodule PoddyclipBackend.Repo.Migrations.RemoveDemoSystem do
  use Ecto.Migration

  def up do
    alter table(:jobs) do
      remove :is_demo
    end

    execute "DELETE FROM users WHERE email = 'demo@munchycow.com'"
  end

  def down do
    alter table(:jobs) do
      add :is_demo, :boolean, default: false
    end

    execute """
    INSERT INTO users (email, hashed_password, seconds_available, subscription_status, plan_id, inserted_at, updated_at)
    SELECT 'demo@munchycow.com', 'demo_no_login', 999999, 'none', p.id, NOW(), NOW()
    FROM plans p WHERE p.name = 'free'
    ON CONFLICT (email) DO NOTHING
    """
  end
end
