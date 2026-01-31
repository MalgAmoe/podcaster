defmodule PoddyclipBackend.Workers.ProcessingWorker do
  @moduledoc """
  Oban worker that starts audio processing jobs on the Rust API.

  Jobs are queued in Postgres and processed with concurrency limited by Oban's
  queue configuration (processing: 4). This replaces direct HTTP calls to the
  Rust API with queued jobs that:

  - Wait in line instead of returning 503 when at capacity
  - Survive Phoenix restarts
  - Have proper visibility and retry handling
  """

  use Oban.Worker,
    queue: :processing,
    max_attempts: 1,  # Don't retry - job state is complex
    unique: [period: 60]  # Prevent duplicate submissions

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Client

  require Logger

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"job_id" => job_id} = args}) do
    Logger.metadata(job_id: job_id, user_id: args["user_id"])

    job = Processing.get_job(job_id)

    cond do
      is_nil(job) ->
        Logger.warning("ProcessingWorker: job not found, skipping")
        :ok

      job.status != :queued ->
        Logger.info("ProcessingWorker: job already started or finished",
          status: job.status
        )
        :ok

      true ->
        start_processing(job, args)
    end
  end

  defp start_processing(job, args) do
    opts = build_opts(args)

    case Client.start_processing(job.id, job.input_s3_key, opts) do
      {:ok, %{"job_id" => rust_job_id}} ->
        Logger.info("ProcessingWorker: started processing",
          rust_job_id: rust_job_id
        )
        Processing.mark_processing(job.id, rust_job_id)
        :ok

      {:error, reason} ->
        Logger.error("ProcessingWorker: failed to start",
          error: inspect(reason)
        )
        Processing.mark_failed(job.id, "Failed to start: #{inspect(reason)}")
        # Return :ok so Oban doesn't retry - we've marked the job as failed
        :ok
    end
  end

  defp build_opts(args) do
    [
      user_id: args["user_id"],
      filename: args["filename"],
      category: args["category"],
      mode: args["mode"],
      strength: args["strength"],
      ai_clean: args["ai_clean"],
      output_format: args["output_format"] || "mp3",
      mp3_bitrate: args["mp3_bitrate"] || 192
    ]
  end
end
