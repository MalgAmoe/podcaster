defmodule PoddyclipBackend.Workers.CleanupOrphanedFiles do
  @moduledoc """
  Oban worker that cleans up orphaned S3 files.

  Finds and deletes S3 files that don't have corresponding job records.
  This can happen if:
  - A job was deleted but S3 cleanup failed
  - Upload completed but job creation failed
  - Manual database operations

  Runs less frequently than job cleanup (e.g., daily) as it requires
  listing S3 objects which can be slow/expensive.
  """

  use Oban.Worker,
    queue: :default,
    max_attempts: 3

  require Logger

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Storage

  import Ecto.Query

  @impl Oban.Worker
  def perform(_job) do
    if not Storage.enabled?() do
      Logger.info("S3 not configured, skipping orphaned file cleanup")
      {:ok, :skipped}
    else
      do_cleanup()
    end
  end

  defp do_cleanup do
    Logger.info("Starting orphaned S3 file cleanup")

    input_deleted = cleanup_orphaned_inputs()
    result_deleted = cleanup_orphaned_results()

    Logger.info("Orphaned file cleanup complete: #{input_deleted} inputs, #{result_deleted} results deleted")

    :ok
  end

  defp cleanup_orphaned_inputs do
    # Get all user IDs that have jobs
    user_ids_with_jobs =
      from(j in Job, select: j.user_id, distinct: true)
      |> Repo.all()
      |> MapSet.new()

    # List all input prefixes (inputs/{user_id}/)
    case list_s3_prefixes("inputs/") do
      {:ok, user_prefixes} ->
        user_prefixes
        |> Enum.reduce(0, fn prefix, acc ->
          # Extract user_id from prefix like "inputs/123/"
          case extract_user_id(prefix, "inputs/") do
            {:ok, user_id} ->
              if MapSet.member?(user_ids_with_jobs, user_id) do
                # User has jobs, check if input matches current job
                cleanup_user_inputs_if_orphaned(user_id) + acc
              else
                # User has no jobs, delete all their inputs
                delete_prefix("inputs/#{user_id}/") + acc
              end

            :error ->
              acc
          end
        end)

      {:error, reason} ->
        Logger.error("Failed to list S3 inputs", error: inspect(reason))
        0
    end
  end

  defp cleanup_orphaned_results do
    # Get all result keys from jobs
    result_keys =
      from(j in Job, where: not is_nil(j.result_s3_key), select: j.result_s3_key)
      |> Repo.all()
      |> MapSet.new()

    # List all result files
    case list_s3_objects("results/") do
      {:ok, objects} ->
        deleted_count =
          objects
          |> Enum.reduce(0, fn key, acc ->
            if MapSet.member?(result_keys, key) do
              acc
            else
              # Orphaned result file
              case Storage.delete(key) do
                {:ok, _} ->
                  acc + 1
                {:error, reason} ->
                  Logger.error("Failed to delete orphaned result",
                    s3_key: key,
                    error: inspect(reason)
                  )
                  acc
              end
            end
          end)

        if deleted_count > 0 do
          Logger.info("Deleted orphaned result files", count: deleted_count)
        end

        deleted_count

      {:error, reason} ->
        Logger.error("Failed to list S3 results", error: inspect(reason))
        0
    end
  end

  defp cleanup_user_inputs_if_orphaned(user_id) do
    # Get the current input key for this user's latest job
    current_input =
      from(j in Job,
        where: j.user_id == ^user_id,
        where: not is_nil(j.input_s3_key),
        order_by: [desc: j.inserted_at],
        limit: 1,
        select: j.input_s3_key
      )
      |> Repo.one()

    # List all inputs for this user
    case list_s3_objects("inputs/#{user_id}/") do
      {:ok, objects} ->
        deleted_count =
          objects
          |> Enum.reduce(0, fn key, acc ->
            if key == current_input do
              acc
            else
              # Old input file, delete it
              case Storage.delete(key) do
                {:ok, _} ->
                  acc + 1
                {:error, reason} ->
                  Logger.error("Failed to delete old input",
                    user_id: user_id,
                    s3_key: key,
                    error: inspect(reason)
                  )
                  acc
              end
            end
          end)

        if deleted_count > 0 do
          Logger.info("Deleted old input files", user_id: user_id, count: deleted_count)
        end

        deleted_count

      {:error, _} ->
        0
    end
  end

  defp list_s3_prefixes(prefix) do
    if Storage.enabled?() do
      bucket = Storage.bucket()

      case ExAws.S3.list_objects(bucket, prefix: prefix, delimiter: "/") |> ExAws.request() do
        {:ok, %{body: %{common_prefixes: prefixes}}} when is_list(prefixes) ->
          {:ok, Enum.map(prefixes, & &1.prefix)}

        {:ok, _} ->
          {:ok, []}

        {:error, reason} ->
          {:error, reason}
      end
    else
      {:error, :s3_not_configured}
    end
  end

  defp list_s3_objects(prefix) do
    if Storage.enabled?() do
      bucket = Storage.bucket()

      case ExAws.S3.list_objects(bucket, prefix: prefix) |> ExAws.request() do
        {:ok, %{body: %{contents: contents}}} when is_list(contents) ->
          {:ok, Enum.map(contents, & &1.key)}

        {:ok, _} ->
          {:ok, []}

        {:error, reason} ->
          {:error, reason}
      end
    else
      {:error, :s3_not_configured}
    end
  end

  defp delete_prefix(prefix) do
    case list_s3_objects(prefix) do
      {:ok, keys} ->
        deleted_count =
          Enum.reduce(keys, 0, fn key, acc ->
            case Storage.delete(key) do
              {:ok, _} ->
                acc + 1
              {:error, reason} ->
                Logger.error("Failed to delete orphaned file",
                  s3_key: key,
                  error: inspect(reason)
                )
                acc
            end
          end)

        if deleted_count > 0 do
          Logger.info("Deleted orphaned files from prefix", prefix: prefix, count: deleted_count)
        end

        deleted_count

      {:error, _} ->
        0
    end
  end

  defp extract_user_id(prefix, base) do
    # "inputs/123/" -> 123
    case String.trim_leading(prefix, base) |> String.trim_trailing("/") |> Integer.parse() do
      {id, ""} -> {:ok, id}
      _ -> :error
    end
  end
end
