defmodule Mix.Tasks.User.ExportData do
  @moduledoc """
  Exports all data for a user (GDPR data portability request).

  Usage:
    mix user.export_data user@example.com

  This will create a JSON file with all user data:
  - Account information
  - Processing job history
  - Minute pack purchases
  - Subscription details

  The file is saved to exports/user_<id>_<timestamp>.json
  """

  use Mix.Task
  import Ecto.Query
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Billing.MinutePack

  @shortdoc "Export user data for GDPR request"

  def run([email]) when is_binary(email) do
    Mix.Task.run("app.start")

    case Accounts.get_user_by_email(email) do
      nil ->
        Mix.shell().error("User not found: #{email}")
        System.halt(1)

      user ->
        export_user_data(user)
    end
  end

  def run(_) do
    Mix.shell().error("Usage: mix user.export_data <email>")
    System.halt(1)
  end

  defp export_user_data(user) do
    user = Repo.preload(user, :plan)

    data = %{
      export_info: %{
        generated_at: DateTime.utc_now(),
        service: "Munchy Cow (munchycow.com)",
        data_controller: "Munchy Cow",
        contact: "privacy@munchycow.com"
      },
      account: export_account(user),
      subscription: export_subscription(user),
      minute_packs: export_minute_packs(user.id),
      processing_jobs: export_jobs(user.id),
      notification_preferences: user.notification_preferences
    }

    # Ensure exports directory exists
    File.mkdir_p!("exports")

    # Generate filename with timestamp
    timestamp = DateTime.utc_now() |> DateTime.to_iso8601(:basic) |> String.replace(~r/[^\d]/, "")
    filename = "exports/user_#{user.id}_#{timestamp}.json"

    # Write JSON file
    json = Jason.encode!(data, pretty: true)
    File.write!(filename, json)

    Mix.shell().info("Data exported successfully to: #{filename}")
    Mix.shell().info("")
    Mix.shell().info("Summary:")
    Mix.shell().info("  User ID: #{user.id}")
    Mix.shell().info("  Email: #{user.email}")
    Mix.shell().info("  Created: #{user.inserted_at}")
    Mix.shell().info("  Jobs: #{length(data.processing_jobs)}")
    Mix.shell().info("  Minute Packs: #{length(data.minute_packs)}")
  end

  defp export_account(user) do
    %{
      id: user.id,
      email: user.email,
      created_at: user.inserted_at,
      confirmed_at: user.confirmed_at,
      seconds_available: user.seconds_available
    }
  end

  defp export_subscription(user) do
    %{
      plan_name: (user.plan && user.plan.name) || "free",
      plan_display_name: (user.plan && user.plan.display_name) || "Free",
      subscription_status: user.subscription_status,
      current_period_ends_at: user.current_period_ends_at,
      # Don't export internal IDs, just indicate if linked
      has_polar_subscription: not is_nil(user.polar_subscription_id)
    }
  end

  defp export_minute_packs(user_id) do
    from(mp in MinutePack,
      where: mp.user_id == ^user_id,
      order_by: [desc: mp.purchased_at],
      select: %{
        id: mp.id,
        seconds_total: mp.seconds_total,
        seconds_remaining: mp.seconds_remaining,
        price_cents: mp.price_cents,
        purchased_at: mp.purchased_at,
        expires_at: mp.expires_at
      }
    )
    |> Repo.all()
    |> Enum.map(fn pack ->
      Map.put(pack, :price_display, "$#{pack.price_cents / 100}")
    end)
  end

  defp export_jobs(user_id) do
    from(j in Job,
      where: j.user_id == ^user_id,
      order_by: [desc: j.inserted_at],
      select: %{
        id: j.id,
        filename: j.filename,
        status: j.status,
        chain: j.chain,
        estimated_seconds: j.estimated_seconds,
        actual_duration_seconds: j.actual_duration_seconds,
        error: j.error,
        created_at: j.inserted_at,
        updated_at: j.updated_at
      }
    )
    |> Repo.all()
    |> Enum.map(fn job ->
      # Convert atom status to string for JSON
      Map.update!(job, :status, &Atom.to_string/1)
    end)
  end
end
