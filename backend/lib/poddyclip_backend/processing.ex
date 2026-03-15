defmodule PoddyclipBackend.Processing do
  @moduledoc """
  Context for audio processing jobs.
  Handles job submission, persistence, and status tracking via webhooks.
  """

  alias PoddyclipBackend.Processing.{Job, Client}
  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.{User, UserNotifier}
  alias PoddyclipBackend.Repo
  import Ecto.Query
  require Logger

  @doc """
  Submit a job for processing using an S3 input key.

  The audio file should already be uploaded to S3. This function:
  1. Creates a job record in the database with status :queued
  2. Enqueues an Oban job to start processing on the Rust API

  The job starts as :queued and transitions to :processing when the Oban
  worker successfully starts it on the Rust API. Oban enforces the
  concurrency limit (4 jobs), so excess jobs wait in line instead of failing.

  ## Options
    * `:strength` - Processing strength: 1-3 (default: 2)
    * `:ai_clean` - Enable AI denoiser
    * `:output_format` - "wav" or "mp3" (default: "mp3")
    * `:mp3_bitrate` - Bitrate for MP3 output (default: 192)
  """
  def submit_job_from_s3(input_s3_key, filename, user_id, opts \\ []) do
    # Create job record first
    job =
      %Job{}
      |> Job.changeset(%{
        filename: filename,
        status: :queued,
        input_s3_key: input_s3_key,
        user_id: user_id,
        estimated_seconds: opts[:estimated_seconds],
        is_demo: opts[:is_demo] || false
      })
      |> Repo.insert!()

    Logger.info("Job submitted",
      job_id: job.id,
      user_id: user_id,
      filename: filename
    )

    # Enqueue Oban job to start processing
    # Oban handles concurrency limits and job queuing
    %{
      job_id: job.id,
      user_id: user_id,
      filename: filename,
      strength: opts[:strength],
      ai_clean: opts[:ai_clean],
      mono: opts[:mono],
      output_format: opts[:output_format],
      mp3_bitrate: opts[:mp3_bitrate]
    }
    |> PoddyclipBackend.Workers.ProcessingWorker.new()
    |> Oban.insert!()

    {:ok, job}
  end

  @doc """
  Get a job by ID.
  """
  def get_job(job_id) do
    Repo.get(Job, job_id)
  end

  @doc """
  Get a job by ID, raises if not found.
  """
  def get_job!(job_id) do
    Repo.get!(Job, job_id)
  end

  @doc """
  Mark a job as processing with the given Rust job ID.
  Called by ProcessingWorker when the Rust API accepts the job.
  """
  def mark_processing(job_id, rust_job_id) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      job ->
        {:ok, updated_job} =
          job
          |> Job.changeset(%{status: :processing, rust_job_id: rust_job_id})
          |> Repo.update()

        Logger.info("Job processing started",
          job_id: job.id,
          user_id: job.user_id,
          rust_job_id: rust_job_id
        )

        broadcast_update(updated_job)
        {:ok, updated_job}
    end
  end

  @doc """
  Mark a job as failed with the given error message.
  Called by ProcessingWorker when the Rust API rejects the job.
  No refund needed - seconds are only deducted on successful completion.
  """
  def mark_failed(job_id, error) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      job ->
        {:ok, updated_job} =
          job
          |> Job.changeset(%{status: :failed, error: error})
          |> Repo.update()

        Logger.error("Job failed",
          job_id: job.id,
          user_id: job.user_id,
          error: error
        )

        broadcast_update(updated_job)
        {:ok, updated_job}
    end
  end

  @doc """
  List all jobs for a user, ordered by most recent first.
  """
  def list_jobs_for_user(user_id) do
    Job
    |> where([j], j.user_id == ^user_id)
    |> order_by([j], desc: j.inserted_at)
    |> Repo.all()
  end

  @doc """
  List completed jobs for a user from the last 7 days.
  Used for job history / "Past Munchings" feature.
  """
  def list_completed_jobs_for_user(user_id) do
    cutoff = DateTime.utc_now() |> DateTime.add(-7, :day)

    Job
    |> where([j], j.user_id == ^user_id)
    |> where([j], j.status == :completed)
    |> where([j], j.inserted_at > ^cutoff)
    |> order_by([j], desc: j.inserted_at)
    |> Repo.all()
  end

  @doc """
  List active (non-completed, non-failed) jobs for a user.
  """
  def list_active_jobs_for_user(user_id) do
    Job
    |> where([j], j.user_id == ^user_id)
    |> where([j], j.status in [:queued, :processing])
    |> order_by([j], desc: j.inserted_at)
    |> Repo.all()
  end

  @doc """
  Get the user's current active job for recovery on page reload.

  Returns the most recent non-dismissed job that is either:
  - Still processing (queued or processing status), OR
  - Completed/failed within the last 24 hours

  Returns nil if no such job exists.
  """
  def get_current_job(user_id) do
    cutoff = DateTime.utc_now() |> DateTime.add(-24, :hour)

    Job
    |> where([j], j.user_id == ^user_id)
    |> where([j], j.dismissed == false)
    |> where([j], j.status in [:queued, :processing] or j.updated_at > ^cutoff)
    |> order_by([j], desc: j.updated_at)
    |> limit(1)
    |> Repo.one()
  end

  @doc """
  Dismiss a completed job so it doesn't show on page reload.
  The job remains in the database for job history.
  """
  def dismiss_job(job_id) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      job ->
        job
        |> Job.changeset(%{dismissed: true})
        |> Repo.update()
    end
  end

  @doc """
  Subscribe to updates for a job.
  Updates are broadcast as {:job_updated, job} messages.
  """
  def subscribe(job_id) do
    Phoenix.PubSub.subscribe(PoddyclipBackend.PubSub, "job:#{job_id}")
  end

  @doc """
  Unsubscribe from job updates.
  """
  def unsubscribe(job_id) do
    Phoenix.PubSub.unsubscribe(PoddyclipBackend.PubSub, "job:#{job_id}")
  end

  @doc """
  Update job status from webhook.
  Called by the Rust API when job status changes.

  When a job fails, refunds the estimated seconds to the user.
  """
  def update_job_status(job_id, params) do
    case get_job(job_id) do
      nil ->
        Logger.warning("Job status update for unknown job", job_id: job_id)
        {:error, :not_found}

      job ->
        # Set user_id in Logger metadata for all subsequent logs
        Logger.metadata(user_id: job.user_id)

        new_status = parse_status(params["status"])
        old_status = job.status

        changes = %{
          status: new_status,
          progress: params["progress"] || %{},
          error: params["error"],
          download_url: params["download_url"],
          result_s3_key: params["result_s3_key"],
          actual_duration_seconds: params["audio_duration_seconds"]
        }

        updated_job =
          job
          |> Job.changeset(changes)
          |> Repo.update!()

        # Log status transitions
        if old_status != new_status do
          log_status_change(updated_job, old_status, new_status, params["error"])
        end

        # Deduct actual seconds on completion (no refunds needed - we only charge on success)
        # Skip billing for demo jobs
        if new_status == :completed and old_status != :completed and not updated_job.is_demo do
          deduct_actual_seconds(updated_job)
        end

        broadcast_update(updated_job)

        # Send email notifications for completed/failed jobs (skip for demo)
        if new_status in [:completed, :failed] and old_status != new_status and not updated_job.is_demo do
          send_job_notification(updated_job, new_status)
        end

        {:ok, updated_job}
    end
  end

  defp log_status_change(job, old_status, :completed, _error) do
    Logger.info("Job completed",
      job_id: job.id,
      user_id: job.user_id,
      prev_status: old_status,
      filename: job.filename
    )
  end

  defp log_status_change(job, old_status, :failed, error) do
    Logger.error("Job failed",
      job_id: job.id,
      user_id: job.user_id,
      prev_status: old_status,
      error: error
    )
  end

  defp log_status_change(job, old_status, new_status, _error) do
    Logger.debug("Job status updated",
      job_id: job.id,
      user_id: job.user_id,
      prev_status: old_status,
      new_status: new_status
    )
  end

  # Deduct actual seconds when job completes successfully
  defp deduct_actual_seconds(%Job{actual_duration_seconds: nil, id: job_id}) do
    Logger.warning("Job completed without actual_duration_seconds", job_id: job_id)
    :ok
  end
  defp deduct_actual_seconds(%Job{
    actual_duration_seconds: actual,
    user_id: user_id,
    id: job_id
  }) when actual > 0 do
    Logger.info("Deducting seconds for completed job",
      job_id: job_id,
      user_id: user_id,
      seconds: actual
    )
    case Accounts.get_user!(user_id) do
      user -> Billing.deduct_seconds(user, actual)
    end
  rescue
    Ecto.NoResultsError -> :ok
  end
  defp deduct_actual_seconds(_), do: :ok

  defp parse_status("queued"), do: :queued
  defp parse_status("processing"), do: :processing
  defp parse_status("completed"), do: :completed
  defp parse_status("failed"), do: :failed
  defp parse_status(_), do: :processing

  # Send email notification for job completion/failure
  defp send_job_notification(job, status) do
    try do
      user = Accounts.get_user!(job.user_id)

      case status do
        :completed ->
          if User.notification_enabled?(user, :job_complete) do
            UserNotifier.deliver_job_complete(user, job)
            Logger.info("Job complete email sent", job_id: job.id, user_id: user.id)
          end

        :failed ->
          if User.notification_enabled?(user, :job_failed) do
            UserNotifier.deliver_job_failed(user, job)
            Logger.info("Job failed email sent", job_id: job.id, user_id: user.id)
          end

        _ ->
          :ok
      end
    rescue
      e ->
        Logger.error("Failed to send job notification email",
          job_id: job.id,
          error: Exception.message(e)
        )
    end
  end

  defp broadcast_update(job) do
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "job:#{job.id}", {:job_updated, job})
  end

  @doc """
  Delete a job from the database.
  Used for cleaning up stale or invalid jobs.
  """
  def delete_job(job_id) do
    case get_job(job_id) do
      nil -> {:error, :not_found}
      job -> Repo.delete(job)
    end
  end

  @doc """
  Cancel a job if possible.
  Marks the job as failed and attempts to delete from Rust API.
  For already-failed jobs, deletes them from the database (cleanup).
  No refund needed - seconds are only deducted on successful completion.
  """
  def cancel_job(job_id) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      %{status: status} = job when status in [:failed, :completed] ->
        # Already finished, just delete from database (cleanup)
        Logger.info("Cleaning up finished job",
          job_id: job.id,
          user_id: job.user_id,
          status: status
        )
        Repo.delete(job)

      job ->
        Logger.info("Job cancelled by user",
          job_id: job.id,
          user_id: job.user_id,
          prev_status: job.status
        )

        # Try to delete from Rust API (best effort)
        if job.rust_job_id do
          Client.delete_job(job.rust_job_id)
        end

        # Mark as failed (no refund needed - seconds only deducted on completion)
        job
        |> Job.changeset(%{status: :failed, error: "Cancelled by user"})
        |> Repo.update()
    end
  end
end
