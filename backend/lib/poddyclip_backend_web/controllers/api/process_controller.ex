defmodule PoddyclipBackendWeb.Api.ProcessController do
  @moduledoc """
  JSON API endpoints for the process page.
  """
  use PoddyclipBackendWeb, :controller

  require Logger

  alias PoddyclipBackend.Billing
  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Processing.Client, as: ProcessingClient
  alias PoddyclipBackend.Storage

  @doc """
  POST /api/presign-upload - Generate S3 presigned PUT URL.

  Request: {"filename": "episode.mp3"}
  Response: {"url": "https://...", "key": "inputs/user_id/input.mp3"}

  Note: Each user has a single input slot. Previous input files are deleted
  before generating a new upload URL.
  """
  def presign_upload(conn, %{"filename" => filename}) do
    user = conn.assigns.current_user

    # Delete any existing input files for this user (single slot per user)
    Storage.delete_user_inputs(user.id)

    key = Storage.input_key(user.id, filename)

    case Storage.presign_upload(key) do
      {:ok, url} ->
        json(conn, %{url: url, key: key})

      {:error, :s3_not_configured} ->
        conn
        |> put_status(503)
        |> json(%{error: "S3 storage not configured"})

      {:error, reason} ->
        conn
        |> put_status(500)
        |> json(%{error: "Failed to generate upload URL: #{inspect(reason)}"})
    end
  end

  def presign_upload(conn, _params) do
    conn
    |> put_status(400)
    |> json(%{error: "Missing filename parameter"})
  end

  @doc """
  POST /api/jobs - Create a processing job.

  Request: {"s3_key": "...", "filename": "...", "duration_seconds": 300}
  Response: {"id": 123, "status": "queued", "filename": "..."}

  The duration_seconds parameter is used to estimate seconds needed. If not provided,
  defaults to 60 seconds as a conservative estimate.
  """
  def create_job(conn, %{"s3_key" => s3_key, "filename" => filename} = params) do
    user = conn.assigns.current_user
    create_job_after_checks(conn, user, s3_key, filename, params)
  end

  def create_job(conn, _params) do
    conn
    |> put_status(400)
    |> json(%{error: "Missing required parameters: s3_key, filename"})
  end

  @doc """
  POST /api/preview - Process a short preview clip synchronously.

  Accepts multipart form data with an `audio` file upload and returns WAV bytes.
  """
  def preview(conn, %{"audio" => %Plug.Upload{} = upload}) do
    with {:ok, audio_bytes} <- File.read(upload.path),
         {:ok, wav_bytes} <- ProcessingClient.preview(audio_bytes, upload.content_type || "audio/wav") do
      conn
      |> put_resp_content_type("audio/wav")
      |> send_resp(200, wav_bytes)
    else
      {:error, :enoent} ->
        conn
        |> put_status(422)
        |> json(%{error: "invalid_input"})

      {:error, :invalid_input} ->
        conn
        |> put_status(422)
        |> json(%{error: "invalid_input"})

      {:error, {:too_long, max_seconds, tolerance_seconds}} ->
        conn
        |> put_status(422)
        |> json(%{
          error: "too_long",
          max_seconds: max_seconds,
          tolerance_seconds: tolerance_seconds
        })

      {:error, :preview_busy} ->
        conn
        |> put_status(503)
        |> json(%{error: "preview_busy"})

      {:error, reason} ->
        Logger.error("Preview processing failed: #{inspect(reason)}")

        conn
        |> put_status(502)
        |> json(%{error: "upstream_error"})
    end
  end

  def preview(conn, _params) do
    conn
    |> put_status(400)
    |> json(%{error: "Missing required parameter: audio"})
  end

  defp create_job_after_checks(conn, user, s3_key, filename, params) do
    estimated_seconds = params["duration_seconds"] || 60

    # Skip billing checks for guest users
    billing_ok =
      if user.is_guest do
        true
      else
        {:ok, user} = Billing.check_subscription_expiry(user)
        Billing.has_seconds?(user, estimated_seconds)
      end

    if not billing_ok do
      conn
      |> put_status(:payment_required)
      |> json(%{
        error: "insufficient_seconds",
        seconds_available: user.seconds_available,
        seconds_needed: estimated_seconds
      })
    else
      opts = [
        estimated_seconds: estimated_seconds
      ]

      try do
        case Processing.submit_job_from_s3(s3_key, filename, user.id, opts) do
          {:ok, job} ->
            json(conn, %{
              id: job.id,
              status: Atom.to_string(job.status),
              filename: job.filename
            })
        end
      rescue
        e ->
          Logger.error("Failed to create job: #{Exception.message(e)}")

          conn
          |> put_status(500)
          |> json(%{error: "Failed to start processing"})
      end
    end
  end

  @doc """
  DELETE /api/jobs/:id - Cancel a job.

  Response: {"ok": true}
  """
  def cancel_job(conn, %{"id" => id}) do
    with_authorized_job(conn, id, fn job ->
      case Processing.cancel_job(job.id) do
        {:ok, _} ->
          json(conn, %{ok: true})

        {:error, :not_found} ->
          conn
          |> put_status(404)
          |> json(%{error: "Job not found"})

        {:error, reason} ->
          conn
          |> put_status(500)
          |> json(%{error: "Failed to cancel job: #{inspect(reason)}"})
      end
    end)
  end

  @doc """
  POST /api/jobs/:id/dismiss - Mark a completed job as dismissed.

  Dismissed jobs won't show on page reload but remain in job history.

  Response: {"ok": true}
  """
  def dismiss_job(conn, %{"id" => id}) do
    with_authorized_job(conn, id, fn job ->
      case Processing.dismiss_job(job.id) do
        {:ok, _} ->
          json(conn, %{ok: true})

        {:error, :not_found} ->
          conn
          |> put_status(404)
          |> json(%{error: "Job not found"})

        {:error, reason} ->
          conn
          |> put_status(500)
          |> json(%{error: "Failed to dismiss job: #{inspect(reason)}"})
      end
    end)
  end

  # Private helper to authorize job access and execute callback
  defp with_authorized_job(conn, job_id, fun) do
    user = conn.assigns.current_user

    case Processing.get_job(job_id) do
      nil ->
        conn
        |> put_status(404)
        |> json(%{error: "Job not found"})

      job when job.user_id != user.id ->
        conn
        |> put_status(403)
        |> json(%{error: "Not authorized"})

      job ->
        fun.(job)
    end
  end

  @doc """
  GET /api/jobs/current - Get the user's current active job (for reload recovery).

  Returns the most recent job that is either still processing or
  completed/failed within the last 24 hours.

  If a completed job's result file no longer exists in S3, the job is
  deleted and null is returned (user sees fresh UI).

  Response: {"job": {...}} or {"job": null}
  """
  def current_job(conn, _params) do
    user = conn.assigns.current_user

    case Processing.get_current_job(user.id) do
      nil ->
        json(conn, %{job: nil})

      %{status: :completed, result_s3_key: key} = job when not is_nil(key) ->
        # Verify result file still exists
        if Storage.exists?(key) do
          json(conn, %{job: job_to_json(job)})
        else
          # File gone, clean up and return fresh state
          Processing.delete_job(job.id)
          json(conn, %{job: nil})
        end

      %{status: :completed} = job ->
        # Completed but no result_s3_key (old job format), clean up
        Processing.delete_job(job.id)
        json(conn, %{job: nil})

      %{status: :failed} = job ->
        # Failed jobs - show error so user can acknowledge
        json(conn, %{job: job_to_json(job)})

      job ->
        # Processing/queued - return as-is
        json(conn, %{job: job_to_json(job)})
    end
  end

  defp job_to_json(job) do
    Job.to_map(job)
    |> Map.put(:original_url, presign_key(job.input_s3_key))
  end

  defp presign_key(nil), do: nil
  defp presign_key(key) do
    case Storage.presign_download(key) do
      {:ok, url} -> url
      _ -> nil
    end
  end

  defp presign_download_with_filename(nil, _filename), do: nil
  defp presign_download_with_filename(key, filename) do
    case Storage.presign_download(key, filename: filename) do
      {:ok, url} -> url
      _ -> nil
    end
  end

  @doc """
  GET /api/jobs/history - Get user's completed jobs from last 7 days.

  Response: {"jobs": [{"id": 123, "filename": "...", "created_at": "..."}]}

  Note: download_url is not included - use GET /api/jobs/:id/download_url to get it on demand.
  """
  def job_history(conn, _params) do
    user = conn.assigns.current_user
    jobs = Processing.list_completed_jobs_for_user(user.id)

    # Get all result keys and batch-check which exist (single S3 call)
    keys = jobs |> Enum.map(& &1.result_s3_key) |> Enum.filter(& &1)
    existing_keys = Storage.filter_existing_keys(keys) |> MapSet.new()

    valid_jobs =
      jobs
      |> Enum.filter(&(&1.result_s3_key && MapSet.member?(existing_keys, &1.result_s3_key)))
      |> Enum.map(fn job ->
        %{
          id: job.id,
          filename: filename_from_s3_key(job.result_s3_key),
          created_at: job.inserted_at
        }
      end)

    json(conn, %{jobs: valid_jobs})
  end

  # Extract filename from S3 key (e.g., "results/123/podcast_processed.mp3" -> "podcast_processed.mp3")
  defp filename_from_s3_key(nil), do: "processed.mp3"
  defp filename_from_s3_key(s3_key), do: s3_key |> String.split("/") |> List.last() || "processed.mp3"

  @doc """
  GET /api/jobs/:id/download_url - Get presigned download URL for a job.

  Response: {"url": "https://..."}

  Only generates the presigned URL when user actually wants to download.
  """
  def download_url(conn, %{"id" => id}) do
    with_authorized_job(conn, id, fn job ->
      cond do
        job.status != :completed ->
          conn
          |> put_status(400)
          |> json(%{error: "Job not completed"})

        is_nil(job.result_s3_key) ->
          conn
          |> put_status(404)
          |> json(%{error: "No result file available"})

        true ->
          processed_name = filename_from_s3_key(job.result_s3_key)
          case presign_download_with_filename(job.result_s3_key, processed_name) do
            nil ->
              conn
              |> put_status(500)
              |> json(%{error: "Failed to generate download URL"})

            url ->
              json(conn, %{url: url})
          end
      end
    end)
  end

  @doc """
  GET /api/user - Get current user info including billing.

  Response: {
    "id": 123,
    "email": "user@example.com",
    "plan": {"name": "free", "display_name": "Free", "seconds": 10800},
    "seconds_available": 720,
    "subscription_status": "none",
    "purchased_seconds": 9000,
    "total_seconds_available": 9720,
    "pack_count": 1
  }
  """
  def current_user(conn, _params) do
    user = conn.assigns.current_user

    # Check if cancelled subscription has expired and downgrade if needed
    {:ok, user} = Billing.check_subscription_expiry(user)
    user = PoddyclipBackend.Repo.preload(user, :plan)

    plan_info =
      if user.plan do
        %{
          name: user.plan.name,
          display_name: user.plan.display_name,
          seconds: user.plan.seconds
        }
      else
        # Fallback for users without a plan (shouldn't happen but be safe)
        %{
          name: "free",
          display_name: "Free",
          seconds: 10_800
        }
      end

    # Get minute pack summary
    pack_summary = Billing.get_pack_summary(user.id)
    total_seconds = Billing.get_total_seconds_available(user)

    json(conn, %{
      id: user.id,
      email: user.email,
      is_guest: user.is_guest,
      completed_jobs_count: user.completed_jobs_count,
      plan: plan_info,
      seconds_available: user.seconds_available,
      subscription_status: user.subscription_status,
      purchased_seconds: pack_summary.total_seconds,
      total_seconds_available: total_seconds,
      pack_count: pack_summary.pack_count
    })
  end
end
