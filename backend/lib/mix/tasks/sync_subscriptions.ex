defmodule Mix.Tasks.SyncSubscriptions do
  @moduledoc """
  Sync subscription status from Polar for users.

  ## Usage

      # Sync a specific user by email
      mix sync_subscriptions user@example.com

      # Sync all users with a polar_customer_id
      mix sync_subscriptions --all
  """
  use Mix.Task

  @shortdoc "Sync subscription status from Polar"

  @impl Mix.Task
  def run(args) do
    Mix.Task.run("app.start")

    alias PoddyclipBackend.{Repo, Billing}
    alias PoddyclipBackend.Accounts.User
    import Ecto.Query

    case args do
      ["--all"] ->
        users =
          User
          |> where([u], not is_nil(u.polar_customer_id))
          |> Repo.all()

        Mix.shell().info("Syncing #{length(users)} users with Polar customer IDs...")

        Enum.each(users, fn user ->
          sync_user(user)
        end)

        Mix.shell().info("Done!")

      [email] ->
        case Repo.get_by(User, email: email) do
          nil ->
            Mix.shell().error("User not found: #{email}")

          user ->
            sync_user(user)
        end

      _ ->
        Mix.shell().info(@moduledoc)
    end
  end

  defp sync_user(user) do
    alias PoddyclipBackend.Billing

    Mix.shell().info("Syncing #{user.email}...")

    case Billing.sync_subscription_from_polar(user) do
      {:ok, updated} ->
        Mix.shell().info("  ✓ status=#{updated.subscription_status}, seconds=#{updated.seconds_available}")

      {:error, :no_access_token} ->
        Mix.shell().error("  ✗ POLAR_ACCESS_TOKEN not configured")

      {:error, reason} ->
        Mix.shell().error("  ✗ #{inspect(reason)}")
    end
  end
end
