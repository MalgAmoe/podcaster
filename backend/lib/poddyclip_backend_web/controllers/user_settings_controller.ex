defmodule PoddyclipBackendWeb.UserSettingsController do
  @moduledoc """
  Email-change confirmation and GDPR data export (rate-limited).
  """
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.{Accounts, RateLimiter}

  import Ecto.Query

  def confirm_email(conn, %{"token" => token}) do
    case Accounts.update_user_email(conn.assigns.current_scope.user, token) do
      {:ok, _user} ->
        conn
        |> put_flash(:info, "Email changed successfully.")
        |> redirect(to: "/users/settings")

      {:error, _} ->
        conn
        |> put_flash(:error, "Email change link is invalid or it has expired.")
        |> redirect(to: "/users/settings")
    end
  end

  def export_data(conn, _params) do
    user = conn.assigns.current_scope.user

    case RateLimiter.check_export(user.id) do
      :ok ->
        RateLimiter.record_export(user.id)
        data = build_export_data(user)
        json = Jason.encode!(data, pretty: true)

        conn
        |> put_resp_content_type("application/json")
        |> put_resp_header("content-disposition", "attachment; filename=\"my-munchy-cow-data.json\"")
        |> send_resp(200, json)

      {:error, seconds_remaining} ->
        minutes = div(seconds_remaining, 60) + 1

        conn
        |> put_flash(:error, "Data export available once per hour. Try again in #{minutes} minutes.")
        |> redirect(to: "/users/settings")
    end
  end

  defp build_export_data(user) do
    alias PoddyclipBackend.Repo
    alias PoddyclipBackend.Processing.Job
    alias PoddyclipBackend.Billing.MinutePack

    user = Repo.preload(user, :plan)

    %{
      export_info: %{
        generated_at: DateTime.utc_now(),
        service: "Munchy Cow (munchycow.com)",
        data_controller: "Munchy Cow",
        contact: "privacy@munchycow.com"
      },
      account: %{
        email: user.email,
        member_since: DateTime.to_date(user.inserted_at)
      },
      subscription: %{
        plan: (user.plan && user.plan.display_name) || "Free",
        status: user.subscription_status,
        current_period_ends: user.current_period_ends_at && DateTime.to_date(user.current_period_ends_at)
      },
      minute_packs: export_minute_packs(user.id),
      processing_jobs: export_jobs(user.id),
      notification_preferences: user.notification_preferences
    }
  end

  defp export_minute_packs(user_id) do
    alias PoddyclipBackend.Repo
    alias PoddyclipBackend.Billing.MinutePack

    from(mp in MinutePack,
      where: mp.user_id == ^user_id,
      order_by: [desc: mp.purchased_at],
      select: %{
        minutes_total: fragment("? / 60", mp.seconds_total),
        minutes_remaining: fragment("? / 60", mp.seconds_remaining),
        purchased: mp.purchased_at,
        expires: mp.expires_at
      }
    )
    |> Repo.all()
    |> Enum.map(fn pack ->
      %{
        minutes_total: pack.minutes_total,
        minutes_remaining: pack.minutes_remaining,
        purchased: DateTime.to_date(pack.purchased),
        expires: DateTime.to_date(pack.expires)
      }
    end)
  end

  defp export_jobs(user_id) do
    alias PoddyclipBackend.Repo
    alias PoddyclipBackend.Processing.Job

    from(j in Job,
      where: j.user_id == ^user_id,
      order_by: [desc: j.inserted_at],
      select: %{
        filename: j.filename,
        status: j.status,
        duration_seconds: j.actual_duration_seconds,
        date: j.inserted_at
      }
    )
    |> Repo.all()
    |> Enum.map(fn job ->
      %{
        filename: job.filename,
        status: Atom.to_string(job.status),
        duration_seconds: job.duration_seconds,
        date: DateTime.to_date(job.date)
      }
    end)
  end
end
