defmodule PoddyclipBackendWeb.Api.DemoController do
  @moduledoc """
  API endpoints for anonymous demo processing.
  Allows visitors to try 30 seconds of audio processing without signing up.
  """
  use PoddyclipBackendWeb, :controller

  require Logger

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.DemoRateLimiter
  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Storage

  @demo_email "demo@munchycow.com"

  @doc """
  POST /api/demo/presign-upload - Generate S3 presigned PUT URL for demo.
  Rate limited to 3 per IP per 24 hours.
  """
  def presign_upload(conn, %{"filename" => filename}) do
    ip = get_client_ip(conn)

    with :ok <- DemoRateLimiter.check_demo(ip),
         {:ok, demo_user} <- get_demo_user() do
      ext = Path.extname(Storage.sanitize_filename(filename))
      key = "inputs/#{demo_user.id}/demo_#{random_id()}#{ext}"

      case Storage.presign_upload(key) do
        {:ok, url} ->
          json(conn, %{url: url, key: key})

        {:error, :s3_not_configured} ->
          conn |> put_status(503) |> json(%{error: "Storage not configured"})

        {:error, reason} ->
          conn |> put_status(500) |> json(%{error: "Upload failed: #{inspect(reason)}"})
      end
    else
      {:error, :rate_limited} ->
        conn |> put_status(429) |> json(%{error: "rate_limited"})

      {:error, :demo_not_available} ->
        conn |> put_status(503) |> json(%{error: "Demo not available"})
    end
  end

  def presign_upload(conn, _params) do
    conn |> put_status(400) |> json(%{error: "Missing filename"})
  end

  @doc """
  POST /api/demo/process - Start processing a demo file.
  """
  def process(conn, %{"s3_key" => s3_key, "filename" => filename}) do
    ip = get_client_ip(conn)

    with :ok <- DemoRateLimiter.check_demo(ip),
         {:ok, demo_user} <- get_demo_user() do
      # Record the attempt now
      DemoRateLimiter.record_demo(ip)

      opts = [strength: 2, ai_clean: true, is_demo: true]

      case Processing.submit_job_from_s3(s3_key, filename, demo_user.id, opts) do
        {:ok, job} ->
          Logger.info("Demo job submitted", job_id: job.id, ip: ip)
          json(conn, %{id: job.id, status: "queued"})
      end
    else
      {:error, :rate_limited} ->
        conn |> put_status(429) |> json(%{error: "rate_limited"})

      {:error, :demo_not_available} ->
        conn |> put_status(503) |> json(%{error: "Demo not available"})
    end
  rescue
    e ->
      Logger.error("Demo processing failed: #{Exception.message(e)}")
      conn |> put_status(500) |> json(%{error: "Processing failed"})
  end

  def process(conn, _params) do
    conn |> put_status(400) |> json(%{error: "Missing s3_key and filename"})
  end

  @doc """
  GET /api/demo/jobs/:id/status - Poll demo job status.
  Only returns info for demo jobs.
  """
  def job_status(conn, %{"id" => id}) do
    case Processing.get_job(id) do
      nil ->
        conn |> put_status(404) |> json(%{error: "Not found"})

      %{is_demo: false} ->
        conn |> put_status(404) |> json(%{error: "Not found"})

      job ->
        response = %{
          status: Atom.to_string(job.status),
          progress: job.progress || %{},
          error: job.error
        }

        response =
          if job.status == :completed do
            response
            |> Map.put(:download_url, presign_key(job.result_s3_key))
            |> Map.put(:original_url, presign_key(job.input_s3_key))
          else
            response
          end

        json(conn, response)
    end
  end

  # Get client IP, preferring Cloudflare's cf-connecting-ip header
  defp get_client_ip(conn) do
    case get_req_header(conn, "cf-connecting-ip") do
      [ip | _] -> ip
      [] ->
        case get_req_header(conn, "x-forwarded-for") do
          [ips | _] -> ips |> String.split(",") |> List.first() |> String.trim()
          [] -> conn.remote_ip |> :inet.ntoa() |> to_string()
        end
    end
  end

  defp get_demo_user do
    case Accounts.get_user_by_email(@demo_email) do
      nil -> {:error, :demo_not_available}
      user -> {:ok, user}
    end
  end

  defp random_id do
    :crypto.strong_rand_bytes(8) |> Base.url_encode64(padding: false)
  end

  defp presign_key(nil), do: nil
  defp presign_key(key) do
    case Storage.presign_download(key) do
      {:ok, url} -> url
      _ -> nil
    end
  end
end
