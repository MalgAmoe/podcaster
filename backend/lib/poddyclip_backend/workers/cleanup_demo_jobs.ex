defmodule PoddyclipBackend.Workers.CleanupDemoJobs do
  @moduledoc """
  Oban cron worker that cleans up demo jobs older than 24 hours.
  Deletes both the S3 files and the job records.
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
    cutoff = DateTime.utc_now() |> DateTime.add(-24, :hour)

    demo_jobs =
      from(j in Job,
        where: j.is_demo == true,
        where: j.inserted_at < ^cutoff,
        select: %{id: j.id, input_s3_key: j.input_s3_key, result_s3_key: j.result_s3_key}
      )
      |> Repo.all()

    if length(demo_jobs) > 0 do
      Logger.info("Cleaning up #{length(demo_jobs)} demo jobs")

      # Delete S3 files
      Enum.each(demo_jobs, fn job ->
        if job.input_s3_key, do: Storage.delete(job.input_s3_key)
        if job.result_s3_key, do: Storage.delete(job.result_s3_key)
      end)

      # Delete job records
      job_ids = Enum.map(demo_jobs, & &1.id)

      {deleted, _} =
        from(j in Job, where: j.id in ^job_ids)
        |> Repo.delete_all()

      Logger.info("Deleted #{deleted} demo jobs and their S3 files")
    end

    :ok
  end
end
