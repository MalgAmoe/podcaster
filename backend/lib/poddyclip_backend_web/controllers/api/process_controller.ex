defmodule PoddyclipBackendWeb.Api.ProcessController do
  @moduledoc """
  JSON API endpoints for the process page.
  """
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Client
  alias PoddyclipBackend.Storage

  @doc """
  GET /api/presets - List available processing presets.
  """
  def presets(conn, _params) do
    case Client.list_presets() do
      {:ok, %{"chain_presets" => presets}} ->
        json(conn, %{presets: Enum.map(presets, & &1["name"])})

      {:ok, body} when is_map(body) ->
        # Handle different response formats
        presets = body["chain_presets"] || body["presets"] || []
        names = Enum.map(presets, fn p -> p["name"] || p end)
        json(conn, %{presets: names})

      {:error, _reason} ->
        # Fallback to default presets
        json(conn, %{presets: ["podcast", "broadcast", "gentle"]})
    end
  end

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

  Request: {"s3_key": "...", "filename": "...", "category": "voice", "mode": "natural", "strength": 3}
  Response: {"id": 123, "status": "queued", "filename": "..."}
  """
  def create_job(conn, %{"s3_key" => s3_key, "filename" => filename} = params) do
    user = conn.assigns.current_user

    opts = [
      category: params["category"] || "voice",
      mode: params["mode"] || "natural",
      strength: params["strength"] || 3,
      ai_clean: params["ai_clean"]
    ]

    case Processing.submit_job_from_s3(s3_key, filename, user.id, opts) do
      {:ok, job} ->
        json(conn, %{
          id: job.id,
          status: Atom.to_string(job.status),
          filename: job.filename
        })

      {:error, {:http_error, status, %{"error" => %{"message" => msg}}}} ->
        conn
        |> put_status(status)
        |> json(%{error: msg})

      {:error, {:http_error, status, body}} when is_binary(body) ->
        conn
        |> put_status(status)
        |> json(%{error: body})

      {:error, reason} when is_binary(reason) ->
        conn
        |> put_status(422)
        |> json(%{error: reason})

      {:error, reason} ->
        conn
        |> put_status(500)
        |> json(%{error: "Failed to start processing: #{inspect(reason)}"})
    end
  end

  def create_job(conn, _params) do
    conn
    |> put_status(400)
    |> json(%{error: "Missing required parameters: s3_key, filename"})
  end

  @doc """
  DELETE /api/jobs/:id - Cancel a job.

  Response: {"ok": true}
  """
  def cancel_job(conn, %{"id" => id}) do
    user = conn.assigns.current_user

    case Processing.get_job(id) do
      nil ->
        conn
        |> put_status(404)
        |> json(%{error: "Job not found"})

      job when job.user_id != user.id ->
        conn
        |> put_status(403)
        |> json(%{error: "Not authorized to cancel this job"})

      job ->
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
    %{
      id: job.id,
      status: Atom.to_string(job.status),
      progress: job.progress || %{},
      filename: job.filename,
      download_url: job.download_url,
      original_url: presign_key(job.input_s3_key),
      error: job.error
    }
  end

  defp presign_key(nil), do: nil
  defp presign_key(key) do
    case Storage.presign_download(key) do
      {:ok, url} -> url
      _ -> nil
    end
  end

  @doc """
  GET /api/user - Get current user info.

  Response: {"email": "user@example.com", "id": 123}
  """
  def current_user(conn, _params) do
    user = conn.assigns.current_user
    json(conn, %{email: user.email, id: user.id})
  end
end
