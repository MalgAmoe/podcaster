defmodule PoddyclipBackend.Workers.CleanupJobs do
  @moduledoc """
  Oban worker that cleans up old jobs and their associated S3 files.

  Runs on a schedule (configured in Oban cron) to:
  1. Delete completed/failed jobs older than retention period
  2. Delete associated S3 files (input and result)
  3. Mark stale processing jobs as failed

  Configuration (config.exs):
    config :poddyclip_backend, :cleanup,
      job_retention_days: 7,
      stale_job_hours: 2
  """

  use Oban.Worker,
    queue: :default,
    max_attempts: 3

  import Ecto.Query
  require Logger

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Storage

  @impl Oban.Worker
  def perform(_job) do
    config = Application.get_env(:poddyclip_backend, :cleanup, [])
    retention_days = Keyword.get(config, :job_retention_days, 7)
    stale_hours = Keyword.get(config, :stale_job_hours, 2)

    Logger.info("Starting job cleanup (retention: #{retention_days} days, stale: #{stale_hours} hours)")

    # 1. Mark stale processing jobs as failed
    stale_count = mark_stale_jobs(stale_hours)

    # 2. Delete old completed/failed jobs
    {deleted_count, s3_deleted} = delete_old_jobs(retention_days)

    Logger.info("Cleanup complete: #{stale_count} stale jobs marked failed, #{deleted_count} jobs deleted, #{s3_deleted} S3 files removed")

    :ok
  end

  defp mark_stale_jobs(hours) do
    cutoff = DateTime.utc_now() |> DateTime.add(-hours, :hour)

    {count, _} =
      from(j in Job,
        where: j.status == :processing,
        where: j.updated_at < ^cutoff
      )
      |> Repo.update_all(set: [
        status: :failed,
        error: "Job timed out (no progress for #{hours} hours)",
        updated_at: DateTime.utc_now()
      ])

    if count > 0 do
      Logger.warning("Marked #{count} stale processing jobs as failed")
    end

    count
  end

  defp delete_old_jobs(days) do
    cutoff = DateTime.utc_now() |> DateTime.add(-days, :day)

    # Find old jobs to delete
    old_jobs =
      from(j in Job,
        where: j.status in [:completed, :failed],
        where: j.updated_at < ^cutoff,
        select: %{id: j.id, input_s3_key: j.input_s3_key, result_s3_key: j.result_s3_key}
      )
      |> Repo.all()

    # Delete S3 files for each job
    s3_deleted =
      old_jobs
      |> Enum.reduce(0, fn job, acc ->
        deleted = delete_job_s3_files(job)
        acc + deleted
      end)

    # Delete the job records
    job_ids = Enum.map(old_jobs, & &1.id)

    {deleted_count, _} =
      from(j in Job, where: j.id in ^job_ids)
      |> Repo.delete_all()

    {deleted_count, s3_deleted}
  end

  defp delete_job_s3_files(job) do
    count = 0

    # Delete input file
    count =
      if job.input_s3_key do
        case Storage.delete(job.input_s3_key) do
          {:ok, _} ->
            count + 1
          {:error, reason} ->
            Logger.error("Failed to delete S3 input",
              job_id: job.id,
              s3_key: job.input_s3_key,
              error: inspect(reason)
            )
            count
        end
      else
        count
      end

    # Delete result file
    if job.result_s3_key do
      case Storage.delete(job.result_s3_key) do
        {:ok, _} ->
          count + 1
        {:error, reason} ->
          Logger.error("Failed to delete S3 result",
            job_id: job.id,
            s3_key: job.result_s3_key,
            error: inspect(reason)
          )
          count
      end
    else
      count
    end
  end
end
