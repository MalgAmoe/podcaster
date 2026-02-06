defmodule PoddyclipBackend.Repo.Migrations.RenameProToMunch do
  use Ecto.Migration

  def up do
    execute """
    UPDATE plans
    SET name = 'munch', display_name = 'Munch Plan'
    WHERE name = 'pro'
    """
  end

  def down do
    execute """
    UPDATE plans
    SET name = 'pro', display_name = 'Pro'
    WHERE name = 'munch'
    """
  end
end
