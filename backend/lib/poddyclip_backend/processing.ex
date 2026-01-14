defmodule PoddyclipBackend.Processing do
  @moduledoc """
  Context for audio processing jobs.
  Handles job submission, persistence, and status tracking via webhooks.
  """

  alias PoddyclipBackend.Processing.{Job, Client}
  alias PoddyclipBackend.Repo
  import Ecto.Query

  @doc """
  Submit a job for processing using an S3 input key.

  The audio file should already be uploaded to S3. This function:
  1. Creates a job record in the database
  2. Notifies the Rust API to start processing (includes webhook URL)

  ## Options
    * `:chain` - Name of the processing chain preset to use
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
        chain: opts[:chain],
        user_id: user_id
      })
      |> Repo.insert!()

    # Notify Rust API to start processing (include filename for output naming)
    case Client.start_processing(job.id, input_s3_key, [{:filename, filename} | opts]) do
      {:ok, %{"job_id" => rust_job_id}} ->
        updated_job =
          job
          |> Job.changeset(%{rust_job_id: rust_job_id, status: :processing})
          |> Repo.update!()

        {:ok, updated_job}

      {:error, reason} ->
        # Job stays queued, can be retried later
        {:error, reason}
    end
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
  List all jobs for a user, ordered by most recent first.
  """
  def list_jobs_for_user(user_id) do
    Job
    |> where([j], j.user_id == ^user_id)
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
  """
  def update_job_status(job_id, params) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      job ->
        changes = %{
          status: parse_status(params["status"]),
          progress: params["progress"] || %{},
          error: params["error"],
          download_url: params["download_url"]
        }

        updated_job =
          job
          |> Job.changeset(changes)
          |> Repo.update!()

        broadcast_update(updated_job)
        {:ok, updated_job}
    end
  end

  defp parse_status("queued"), do: :queued
  defp parse_status("processing"), do: :processing
  defp parse_status("completed"), do: :completed
  defp parse_status("failed"), do: :failed
  defp parse_status(_), do: :processing

  defp broadcast_update(job) do
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "job:#{job.id}", {:job_updated, job})
  end

  @doc """
  Cancel a job if possible.
  Marks the job as failed and attempts to delete from Rust API.
  """
  def cancel_job(job_id) do
    case get_job(job_id) do
      nil ->
        {:error, :not_found}

      job ->
        # Try to delete from Rust API (best effort)
        if job.rust_job_id do
          Client.delete_job(job.rust_job_id)
        end

        # Mark as failed
        job
        |> Job.changeset(%{status: :failed, error: "Cancelled by user"})
        |> Repo.update()
    end
  end
end
