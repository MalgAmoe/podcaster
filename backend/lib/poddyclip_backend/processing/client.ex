defmodule PoddyclipBackend.Processing.Client do
  @moduledoc """
  HTTP client for communicating with the poddyclip-api Rust service.
  """

  require Logger

  @default_base_url "http://localhost:3000"

  def base_url do
    Application.get_env(:poddyclip_backend, :poddyclip_api_url, @default_base_url)
  end

  def webhook_url do
    base = Application.get_env(:poddyclip_backend, :webhook_base_url, "http://localhost:4000")
    "#{base}/api/internal/jobs"
  end

  defp api_key do
    Application.get_env(:poddyclip_backend, :api_key)
  end

  defp auth_headers do
    case api_key() do
      nil -> []
      key -> [{"x-api-key", key}]
    end
  end

  @doc """
  Check if the poddyclip-api service is healthy.
  """
  def health do
    case Req.get("#{base_url()}/health") do
      {:ok, %{status: 200, body: body}} ->
        {:ok, body}

      {:ok, %{status: status, body: body}} ->
        {:error, {:http_error, status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @doc """
  List available processing presets.
  """
  def list_presets do
    case Req.get("#{base_url()}/presets") do
      {:ok, %{status: 200, body: body}} ->
        {:ok, body}

      {:ok, %{status: status, body: body}} ->
        {:error, {:http_error, status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @doc """
  Start processing a job using an S3 input key.

  This is the preferred method - the audio file is already in S3,
  and the Rust API will download it from there.

  ## Options
    * `:filename` - Original filename (for output naming)
    * `:chain` - Name of the processing chain preset to use
    * `:output_format` - "wav" or "mp3" (default: "mp3")
    * `:mp3_bitrate` - Bitrate for MP3 output (default: 192)
  """
  def start_processing(phoenix_job_id, input_s3_key, opts \\ []) do
    config = %{
      phoenix_job_id: phoenix_job_id,
      input_s3_key: input_s3_key,
      filename: Keyword.get(opts, :filename),
      chain: Keyword.get(opts, :chain),
      output_format: Keyword.get(opts, :output_format, "mp3"),
      mp3_bitrate: Keyword.get(opts, :mp3_bitrate, 192),
      webhook_url: webhook_url(),
      webhook_secret: Application.get_env(:poddyclip_backend, :webhook_secret)
    }

    case Req.post("#{base_url()}/jobs",
           json: config,
           headers: auth_headers(),
           receive_timeout: 30_000
         ) do
      {:ok, %{status: 200, body: body}} ->
        {:ok, body}

      {:ok, %{status: 202, body: body}} ->
        {:ok, body}

      {:ok, %{status: status, body: body}} ->
        {:error, {:http_error, status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @doc """
  Delete a job.
  """
  def delete_job(job_id) do
    case Req.delete("#{base_url()}/jobs/#{job_id}", headers: auth_headers()) do
      {:ok, %{status: 200, body: body}} ->
        {:ok, body}

      {:ok, %{status: 404}} ->
        {:error, :not_found}

      {:ok, %{status: status, body: body}} ->
        {:error, {:http_error, status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end

end
