defmodule Mix.Tasks.Polar.ImportSubscribers do
  @moduledoc """
  Import active subscribers from Polar into the local database.

  This is a dev recovery tool for when you wipe your DB but keep Polar subscriptions.
  It fetches all active subscriptions from Polar and creates missing users.

  ## Usage

      mix polar.import_subscribers

  ## Required Token Scopes

  Your POLAR_ACCESS_TOKEN needs:
  - `subscriptions:read`
  - `customers:read`
  """
  use Mix.Task

  @shortdoc "Import active subscribers from Polar (dev recovery)"

  @impl Mix.Task
  def run(_args) do
    Mix.Task.run("app.start")

    alias PoddyclipBackend.{Repo, Billing, Polar}
    alias PoddyclipBackend.Accounts.User

    Mix.shell().info("Fetching active subscriptions from Polar...")

    case Polar.list_active_subscriptions() do
      {:ok, subscriptions} ->
        Mix.shell().info("Found #{length(subscriptions)} active subscriptions\n")

        pro_plan = Billing.get_plan_by_name("pro")

        if is_nil(pro_plan) do
          Mix.shell().error("Pro plan not found in database. Run seeds first.")
        else
          Enum.each(subscriptions, fn sub ->
            import_subscription(sub, pro_plan)
          end)

          Mix.shell().info("\nDone!")
        end

      {:error, :no_access_token} ->
        Mix.shell().error("POLAR_ACCESS_TOKEN not configured")

      {:error, reason} ->
        Mix.shell().error("Failed to fetch subscriptions: #{inspect(reason)}")
    end
  end

  defp import_subscription(sub, pro_plan) do
    alias PoddyclipBackend.{Repo, Billing, Polar}
    alias PoddyclipBackend.Accounts.User

    customer_id = sub["customer_id"]

    # Fetch customer details to get email
    case Polar.get_customer(customer_id) do
      {:ok, customer} ->
        email = customer["email"]
        Mix.shell().info("Processing #{email}...")

        # Check if user exists
        case Repo.get_by(User, email: email) do
          nil ->
            # Create user
            create_user_from_polar(email, customer_id, sub, pro_plan)

          user ->
            # User exists, just sync
            sync_existing_user(user, customer_id, sub)
        end

      {:error, reason} ->
        Mix.shell().error("  ✗ Failed to fetch customer #{customer_id}: #{inspect(reason)}")
    end
  end

  defp create_user_from_polar(email, customer_id, sub, pro_plan) do
    alias PoddyclipBackend.Repo
    alias PoddyclipBackend.Accounts.User

    period_end = parse_datetime(sub["current_period_end"])

    changeset =
      %User{}
      |> User.email_changeset(%{email: email})
      |> Ecto.Changeset.put_change(:polar_customer_id, customer_id)
      |> Ecto.Changeset.put_change(:polar_subscription_id, sub["id"])
      |> Ecto.Changeset.put_change(:subscription_status, "active")
      |> Ecto.Changeset.put_change(:current_period_ends_at, period_end)
      |> Ecto.Changeset.put_change(:plan_id, pro_plan.id)
      |> Ecto.Changeset.put_change(:seconds_available, pro_plan.seconds)

    case Repo.insert(changeset) do
      {:ok, user} ->
        Mix.shell().info("  ✓ Created user with #{user.seconds_available} seconds")

      {:error, changeset} ->
        errors = Ecto.Changeset.traverse_errors(changeset, fn {msg, _} -> msg end)
        Mix.shell().error("  ✗ Failed to create: #{inspect(errors)}")
    end
  end

  defp sync_existing_user(user, customer_id, _sub) do
    alias PoddyclipBackend.Billing

    # Update polar_customer_id if not set
    user =
      if is_nil(user.polar_customer_id) do
        {:ok, updated} = Billing.update_subscription(user, %{polar_customer_id: customer_id})
        updated
      else
        user
      end

    case Billing.sync_subscription_from_polar(user) do
      {:ok, updated} ->
        Mix.shell().info("  ✓ Synced: status=#{updated.subscription_status}, seconds=#{updated.seconds_available}")

      {:error, reason} ->
        Mix.shell().error("  ✗ Sync failed: #{inspect(reason)}")
    end
  end

  defp parse_datetime(nil), do: nil
  defp parse_datetime(datetime_str) when is_binary(datetime_str) do
    case DateTime.from_iso8601(datetime_str) do
      {:ok, dt, _offset} -> DateTime.truncate(dt, :second)
      _ -> nil
    end
  end
end
