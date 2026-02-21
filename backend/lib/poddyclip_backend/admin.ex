defmodule PoddyclipBackend.Admin do
  @moduledoc """
  Admin context for operational visibility.

  Provides query functions for:
  - Job statistics (queued, processing, completed, failed)
  - User statistics (by plan, subscription status)
  - System health checks (database, S3, Rust API, Oban)
  """

  import Ecto.Query
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Accounts.User

  # ----- Job Stats -----

  @doc """
  Returns job statistics.

  ## Returns

      %{
        current: %{queued: 2, processing: 1},
        last_24h: %{completed: 47, failed: 3, success_rate: 94.0}
      }
  """
  def job_stats do
    %{
      current: %{
        queued: count_jobs_by_status(:queued),
        processing: count_jobs_by_status(:processing)
      },
      last_24h: last_24h_stats()
    }
  end

  defp count_jobs_by_status(status) do
    Job
    |> where([j], j.status == ^status)
    |> Repo.aggregate(:count)
  end

  defp last_24h_stats do
    cutoff = DateTime.utc_now() |> DateTime.add(-24, :hour)

    completed =
      Job
      |> where([j], j.status == :completed and j.updated_at > ^cutoff)
      |> Repo.aggregate(:count)

    failed =
      Job
      |> where([j], j.status == :failed and j.updated_at > ^cutoff)
      |> Repo.aggregate(:count)

    total = completed + failed
    success_rate = if total > 0, do: Float.round(completed / total * 100, 1), else: 100.0

    %{
      completed: completed,
      failed: failed,
      success_rate: success_rate
    }
  end

  @doc """
  Returns recent failed jobs with error messages.

  ## Options

    * `:limit` - Maximum number of errors to return (default: 10)
  """
  def recent_errors(limit \\ 10) do
    cutoff = DateTime.utc_now() |> DateTime.add(-24, :hour)

    jobs =
      Job
      |> where([j], j.status == :failed and j.updated_at > ^cutoff)
      |> order_by([j], desc: j.updated_at)
      |> limit(^limit)
      |> Repo.all()

    errors =
      Enum.map(jobs, fn job ->
        %{
          job_id: job.id,
          filename: job.filename,
          error: job.error,
          user_id: job.user_id,
          failed_at: job.updated_at
        }
      end)

    total =
      Job
      |> where([j], j.status == :failed and j.updated_at > ^cutoff)
      |> Repo.aggregate(:count)

    %{errors: errors, total_24h: total}
  end

  @doc """
  Returns currently active (queued or processing) jobs.
  """
  def active_jobs do
    Job
    |> where([j], j.status in [:queued, :processing])
    |> order_by([j], asc: j.inserted_at)
    |> Repo.all()
    |> Enum.map(fn job ->
      %{
        job_id: job.id,
        filename: job.filename,
        status: job.status,
        user_id: job.user_id,
        progress: job.progress,
        started_at: job.inserted_at
      }
    end)
  end

  # ----- User Stats -----

  @doc """
  Returns user statistics by plan and subscription status.

  ## Returns

      %{
        total: 150,
        by_plan: %{"free" => 120, "munch" => 30},
        by_subscription: %{"none" => 120, "active" => 25, "cancelled" => 5},
        total_seconds_available: 720000
      }
  """
  def user_stats do
    total = Repo.aggregate(User, :count)

    by_plan =
      User
      |> join(:left, [u], p in assoc(u, :plan))
      |> group_by([u, p], p.name)
      |> select([u, p], {coalesce(p.name, "none"), count(u.id)})
      |> Repo.all()
      |> Map.new()

    by_subscription =
      User
      |> group_by([u], u.subscription_status)
      |> select([u], {u.subscription_status, count(u.id)})
      |> Repo.all()
      |> Map.new()

    total_seconds_available =
      User
      |> select([u], sum(u.seconds_available))
      |> Repo.one() || 0

    %{
      total: total,
      by_plan: by_plan,
      by_subscription: by_subscription,
      total_seconds_available: total_seconds_available
    }
  end

  # ----- Health Checks -----

  @doc """
  Performs system health checks.

  Checks:
  - Database connectivity
  - S3 storage reachability
  - Rust API reachability
  - Oban queue status

  ## Returns

      %{
        status: "healthy" | "degraded" | "unhealthy",
        checks: %{
          database: %{status: "ok", latency_ms: 2},
          s3: %{status: "ok"},
          rust_api: %{status: "ok", latency_ms: 45},
          oban: %{status: "ok", queues: %{...}}
        },
        timestamp: ~U[2026-01-26 13:30:00Z]
      }
  """
  def health_check do
    checks = %{
      database: check_database(),
      s3: check_s3(),
      rust_api: check_rust_api(),
      oban: check_oban()
    }

    overall_status = determine_overall_status(checks)

    %{
      status: overall_status,
      checks: checks,
      timestamp: DateTime.utc_now()
    }
  end

  defp check_database do
    start = System.monotonic_time(:millisecond)

    try do
      Repo.query!("SELECT 1")
      latency = System.monotonic_time(:millisecond) - start
      %{status: "ok", latency_ms: latency}
    rescue
      e ->
        %{status: "error", error: Exception.message(e)}
    end
  end

  defp check_s3 do
    s3_config = Application.get_env(:poddyclip_backend, :s3, [])

    if Keyword.get(s3_config, :enabled, false) do
      bucket = Keyword.get(s3_config, :bucket, "poddyclip")

      try do
        # Try to list objects (limited to 1) to verify connectivity
        case ExAws.S3.list_objects(bucket, max_keys: 1) |> ExAws.request() do
          {:ok, _} -> %{status: "ok"}
          {:error, reason} -> %{status: "error", error: inspect(reason)}
        end
      rescue
        e ->
          %{status: "error", error: Exception.message(e)}
      end
    else
      %{status: "disabled", message: "S3 not configured"}
    end
  end

  defp check_rust_api do
    # Get API base URL from config or default
    api_base = System.get_env("PODDYCLIP_API_URL", "http://localhost:3000")
    health_url = "#{api_base}/health"

    start = System.monotonic_time(:millisecond)

    try do
      case Req.get(health_url, receive_timeout: 5000) do
        {:ok, %{status: status}} when status in 200..299 ->
          latency = System.monotonic_time(:millisecond) - start
          %{status: "ok", latency_ms: latency}

        {:ok, %{status: status}} ->
          %{status: "error", error: "HTTP #{status}"}

        {:error, reason} ->
          %{status: "error", error: inspect(reason)}
      end
    rescue
      e ->
        %{status: "error", error: Exception.message(e)}
    end
  end

  defp check_oban do
    try do
      # Get Oban queue status
      queues = Oban.config().queues

      queue_status =
        queues
        |> Enum.map(fn {name, opts} ->
          # opts can be an integer (limit) or keyword list with :limit key
          limit = if is_integer(opts), do: opts, else: Keyword.get(opts, :limit, 0)

          # Count executing and available jobs for each queue
          executing =
            Oban.Job
            |> where([j], j.queue == ^to_string(name) and j.state == "executing")
            |> Repo.aggregate(:count)

          available =
            Oban.Job
            |> where([j], j.queue == ^to_string(name) and j.state == "available")
            |> Repo.aggregate(:count)

          {name, %{limit: limit, executing: executing, available: available}}
        end)
        |> Map.new()

      %{status: "ok", queues: queue_status}
    rescue
      e ->
        %{status: "error", error: Exception.message(e)}
    end
  end

  defp determine_overall_status(checks) do
    statuses = Enum.map(checks, fn {_key, check} -> check.status end)

    cond do
      Enum.all?(statuses, &(&1 == "ok" or &1 == "disabled")) -> "healthy"
      Enum.any?(statuses, &(&1 == "error")) -> "degraded"
      true -> "healthy"
    end
  end
end
