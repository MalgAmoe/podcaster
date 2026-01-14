defmodule PoddyclipBackend.Processing.Client do
  @moduledoc """
  HTTP client for communicating with the poddyclip-api Rust service.
  """

  require Logger

  @default_base_url "http://localhost:3000"
  @timeout 600_000  # 10 minutes for processing

  def base_url do
    Application.get_env(:poddyclip_backend, :poddyclip_api_url, @default_base_url)
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
  Submit an audio file for processing.

  ## Options
    * `:chain` - Name of the processing chain preset to use
    * `:output_format` - "wav" or "mp3" (default: "mp3")
    * `:mp3_bitrate` - Bitrate for MP3 output (default: 192)
  """
  def process_audio(audio_binary, filename, opts \\ []) do
    config = build_config(opts)

    multipart =
      Multipart.new()
      |> Multipart.add_part(Multipart.Part.file_content_field(filename, audio_binary, "audio"))
      |> Multipart.add_part(Multipart.Part.text_field(Jason.encode!(config), "config"))

    body = Multipart.body_binary(multipart)
    content_type = Multipart.content_type(multipart, "multipart/form-data")

    case Req.post("#{base_url()}/process",
           body: body,
           headers: [{"content-type", content_type}],
           receive_timeout: @timeout
         ) do
      {:ok, %{status: 200, body: body}} ->
        {:ok, body}

      {:ok, %{status: status, body: body}} ->
        {:error, {:http_error, status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @doc """
  Get the status of a processing job.
  """
  def get_job_status(job_id) do
    case Req.get("#{base_url()}/jobs/#{job_id}") do
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

  @doc """
  Download the processed audio result.
  """
  def get_job_result(job_id) do
    case Req.get("#{base_url()}/jobs/#{job_id}/result", receive_timeout: @timeout) do
      {:ok, %{status: 200, body: body, headers: headers}} ->
        content_type =
          case headers do
            %{"content-type" => [v | _]} -> v
            %{"content-type" => v} when is_binary(v) -> v
            _ -> "audio/mpeg"
          end

        {:ok, body, content_type}

      {:ok, %{status: 404}} ->
        {:error, :not_found}

      {:ok, %{status: 409, body: body}} ->
        # Job not completed yet
        {:error, {:not_ready, body}}

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
    case Req.delete("#{base_url()}/jobs/#{job_id}") do
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

  # Build config map from options
  defp build_config(opts) do
    config = %{}

    config =
      if chain = Keyword.get(opts, :chain) do
        Map.put(config, :chain, chain)
      else
        config
      end

    config =
      if format = Keyword.get(opts, :output_format) do
        Map.put(config, :output_format, format)
      else
        config
      end

    config =
      if bitrate = Keyword.get(opts, :mp3_bitrate) do
        Map.put(config, :mp3_bitrate, bitrate)
      else
        config
      end

    config
  end
end
