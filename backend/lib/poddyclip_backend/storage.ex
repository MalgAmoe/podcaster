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

  Options:
    - `:filename` - Custom filename for Content-Disposition header
  """
  def presign_download(key, opts \\ []) do
    if enabled?() do
      config = ExAws.Config.new(:s3)
      expires_in = opts[:expires_in] || 3600

      query_params =
        case opts[:filename] do
          nil -> []
          name -> [{"response-content-disposition", "attachment; filename=\"#{name}\""}]
        end

      ExAws.S3.presigned_url(config, :get, bucket(), key,
        expires_in: expires_in,
        query_params: query_params
      )
    else
      {:error, :s3_not_configured}
    end
  end

  @doc """
  Generate the S3 key for a user's input file.
  Uses a fixed name per user (single slot) with the file extension preserved.
  Sanitizes filename to prevent path traversal attacks.
  """
  def input_key(user_id, filename) do
    safe_name = sanitize_filename(filename)
    ext = Path.extname(safe_name)
    "inputs/#{user_id}/input#{ext}"
  end

  @doc """
  Sanitize a filename to prevent path traversal and other attacks.
  - Removes directory components (../, ./, etc.)
  - Only allows alphanumeric, dots, hyphens, and underscores
  - Limits length to 255 characters
  """
  def sanitize_filename(filename) when is_binary(filename) do
    filename
    |> Path.basename()                           # Remove any directory components
    |> String.replace(~r/\.\./, "")              # Remove .. sequences
    |> String.replace(~r/[^\w.\-]/, "_")         # Only allow safe characters
    |> String.slice(0, 255)                      # Limit length
    |> case do
      "" -> "unnamed"                            # Fallback if empty
      name -> name
    end
  end

  def sanitize_filename(_), do: "unnamed"

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

  @doc """
  List all existing keys under a prefix.

  Returns a list of keys that exist in S3.
  """
  def list_keys(prefix) do
    if enabled?() do
      case ExAws.S3.list_objects(bucket(), prefix: prefix) |> ExAws.request() do
        {:ok, %{body: %{contents: contents}}} when is_list(contents) ->
          Enum.map(contents, & &1.key)

        {:ok, _} ->
          []

        {:error, _} ->
          []
      end
    else
      []
    end
  end

  @doc """
  Filter a list of keys to only those that exist in S3.

  Uses list_objects with a common prefix for efficiency (single S3 API call).
  Falls back to individual checks if keys don't share a prefix.
  """
  def filter_existing_keys([]), do: []

  def filter_existing_keys(keys) do
    if enabled?() do
      # Find common prefix (e.g., "results/123/" for user's result files)
      case find_common_prefix(keys) do
        nil ->
          # No common prefix, fall back to parallel HEAD requests
          keys
          |> Task.async_stream(&{&1, exists?(&1)}, max_concurrency: 10, timeout: 5000)
          |> Enum.flat_map(fn
            {:ok, {key, true}} -> [key]
            _ -> []
          end)

        prefix ->
          # Single list_objects call with common prefix
          existing_set = list_keys(prefix) |> MapSet.new()
          Enum.filter(keys, &MapSet.member?(existing_set, &1))
      end
    else
      []
    end
  end

  # Find common prefix for a list of keys (e.g., "results/123/")
  defp find_common_prefix(keys) do
    prefixes =
      keys
      |> Enum.map(fn key ->
        case String.split(key, "/", parts: 3) do
          [a, b, _rest] -> "#{a}/#{b}/"
          _ -> nil
        end
      end)
      |> Enum.uniq()

    case prefixes do
      [prefix] when not is_nil(prefix) -> prefix
      _ -> nil
    end
  end

end
