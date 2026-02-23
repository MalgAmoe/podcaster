defmodule PoddyclipBackendWeb.AdminApiController do
  @moduledoc """
  Admin API controller for operational visibility.

  Provides JSON endpoints for:
  - System health checks
  - Job statistics
  - Recent errors
  - User statistics

  All endpoints require admin authentication (HTTP Basic Auth).
  """

  use PoddyclipBackendWeb, :controller
  alias PoddyclipBackend.Admin

  @doc """
  GET /admin/api/health

  Returns overall system health status including database, S3, Rust API, and Oban.
  """
  def health(conn, _params) do
    json(conn, Admin.health_check())
  end

  @doc """
  GET /admin/api/jobs/stats

  Returns job statistics including current queue depths and 24h success rate.
  """
  def job_stats(conn, _params) do
    stats = Admin.job_stats()

    json(conn, %{
      current: stats.current,
      last_24h: stats.last_24h,
      timestamp: DateTime.utc_now()
    })
  end

  @doc """
  GET /admin/api/errors?limit=10

  Returns recent failed jobs with error messages.
  """
  def errors(conn, params) do
    limit = parse_limit(params["limit"], 10)
    result = Admin.recent_errors(limit)
    json(conn, result)
  end

  @doc """
  GET /admin/api/users/stats

  Returns user statistics by plan and subscription status.
  """
  def user_stats(conn, _params) do
    stats = Admin.user_stats()

    json(conn, %{
      users: stats,
      timestamp: DateTime.utc_now()
    })
  end

  @doc """
  GET /admin/api/jobs/active

  Returns currently active (queued or processing) jobs.
  """
  def active_jobs(conn, _params) do
    jobs = Admin.active_jobs()

    json(conn, %{
      jobs: jobs,
      count: length(jobs),
      timestamp: DateTime.utc_now()
    })
  end

  @doc """
  GET /admin/api/billing/stats

  Returns billing statistics: upcoming worker actions, current state, and recent activity.
  """
  def billing_stats(conn, _params) do
    stats = Admin.billing_stats()

    json(conn, %{
      billing: stats,
      timestamp: DateTime.utc_now()
    })
  end

  defp parse_limit(nil, default), do: default
  defp parse_limit(value, default) when is_binary(value) do
    case Integer.parse(value) do
      {n, _} when n > 0 and n <= 100 -> n
      _ -> default
    end
  end
  defp parse_limit(value, _default) when is_integer(value) and value > 0 and value <= 100, do: value
  defp parse_limit(_, default), do: default
end
