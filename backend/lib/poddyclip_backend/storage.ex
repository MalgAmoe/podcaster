defmodule PoddyclipBackend.Storage do
  @moduledoc """
  S3 storage client for handling audio file uploads and downloads.
  Uses presigned URLs for direct browser-to-S3 uploads.
  """

  @doc """
  Check if S3 storage is enabled.
  """
  def enabled? do
    Application.get_env(:poddyclip_backend, :s3, [])[:enabled] == true
  end

  @doc """
  Get the configured bucket name.
  """
  def bucket do
    Application.get_env(:poddyclip_backend, :s3, [])[:bucket] || "poddyclip"
  end

  @doc """
  Generate a presigned URL for uploading an input file.

  Returns `{:ok, %{upload_url: url, key: key}}` on success.
  The key will be in the format: `inputs/{user_id}/{timestamp}_{filename}`
  """
  def presign_upload(user_id, filename) do
    if enabled?() do
      sanitized_filename = sanitize_filename(filename)
      key = "inputs/#{user_id}/#{:erlang.unique_integer([:positive])}_#{sanitized_filename}"

      config = ExAws.Config.new(:s3)

      case ExAws.S3.presigned_url(config, :put, bucket(), key,
             expires_in: 3600,
             query_params: [{"Content-Type", "application/octet-stream"}]
           ) do
        {:ok, url} ->
          {:ok, %{upload_url: url, key: key}}

        {:error, reason} ->
          {:error, reason}
      end
    else
      {:error, :s3_not_configured}
    end
  end

  @doc """
  Generate a presigned URL for downloading a file.
  """
  def presign_download(key, expires_in \\ 3600) do
    if enabled?() do
      config = ExAws.Config.new(:s3)
      ExAws.S3.presigned_url(config, :get, bucket(), key, expires_in: expires_in)
    else
      {:error, :s3_not_configured}
    end
  end

  @doc """
  Upload a file to S3.

  Returns `{:ok, key}` on success.
  """
  def upload(user_id, filename, content) do
    if enabled?() do
      sanitized_filename = sanitize_filename(filename)
      key = "inputs/#{user_id}/#{:erlang.unique_integer([:positive])}_#{sanitized_filename}"

      case ExAws.S3.put_object(bucket(), key, content)
           |> ExAws.request() do
        {:ok, _} -> {:ok, key}
        {:error, reason} -> {:error, reason}
      end
    else
      {:error, :s3_not_configured}
    end
  end

  @doc """
  Delete a file from S3.
  """
  def delete(key) do
    if enabled?() do
      ExAws.S3.delete_object(bucket(), key)
      |> ExAws.request()
    else
      {:error, :s3_not_configured}
    end
  end

  @doc """
  Check if a file exists in S3.
  """
  def exists?(key) do
    if enabled?() do
      case ExAws.S3.head_object(bucket(), key) |> ExAws.request() do
        {:ok, _} -> true
        {:error, _} -> false
      end
    else
      false
    end
  end

  # Sanitize filename to be safe for S3 keys
  defp sanitize_filename(filename) do
    filename
    |> String.replace(~r/[^\w\.\-]/, "_")
    |> String.slice(0, 200)
  end
end
