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

  Returns `{:ok, url}` on success.
  """
  def presign_upload(key) do
    if enabled?() do
      config = ExAws.Config.new(:s3)

      case ExAws.S3.presigned_url(config, :put, bucket(), key,
             expires_in: 3600,
             query_params: [{"Content-Type", "application/octet-stream"}]
           ) do
        {:ok, url} ->
          {:ok, url}

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
  Generate the S3 key for a user's input file.
  Uses a fixed name per user (single slot) with the file extension preserved.
  """
  def input_key(user_id, filename) do
    ext = Path.extname(filename)
    "inputs/#{user_id}/input#{ext}"
  end

  @doc """
  Delete all input files for a user.
  Called before uploading to ensure only one input file exists.
  """
  def delete_user_inputs(user_id) do
    if enabled?() do
      prefix = "inputs/#{user_id}/"

      case ExAws.S3.list_objects(bucket(), prefix: prefix) |> ExAws.request() do
        {:ok, %{body: %{contents: contents}}} when is_list(contents) ->
          # Delete each object found
          Enum.each(contents, fn %{key: key} ->
            ExAws.S3.delete_object(bucket(), key) |> ExAws.request()
          end)
          :ok

        {:ok, _} ->
          # No objects found, nothing to delete
          :ok

        {:error, reason} ->
          {:error, reason}
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

end
