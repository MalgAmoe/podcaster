defmodule PoddyclipBackend.Processing.Job do
  @moduledoc """
  Represents a processing job submitted to poddyclip-api.
  """

  @type status :: :queued | :processing | :completed | :failed

  @type t :: %__MODULE__{
          id: String.t(),
          rust_job_id: String.t() | nil,
          filename: String.t(),
          status: status(),
          progress: map(),
          error: String.t() | nil,
          created_at: DateTime.t(),
          updated_at: DateTime.t(),
          user_id: String.t() | nil,
          result_path: String.t() | nil
        }

  defstruct [
    :id,
    :rust_job_id,
    :filename,
    :status,
    :progress,
    :error,
    :created_at,
    :updated_at,
    :user_id,
    :result_path
  ]

  def new(filename, opts \\ []) do
    now = DateTime.utc_now()

    %__MODULE__{
      id: generate_id(),
      rust_job_id: nil,
      filename: filename,
      status: :queued,
      progress: %{"stage" => "pending", "percent_complete" => 0},
      error: nil,
      created_at: now,
      updated_at: now,
      user_id: Keyword.get(opts, :user_id),
      result_path: nil
    }
  end

  def update_from_rust_status(job, rust_status) do
    status =
      case rust_status["status"] do
        "queued" -> :queued
        "processing" -> :processing
        "completed" -> :completed
        "failed" -> :failed
        _ -> job.status
      end

    progress = rust_status["progress"] || job.progress

    %{job |
      status: status,
      progress: progress,
      error: rust_status["error"],
      updated_at: DateTime.utc_now()
    }
  end

  defp generate_id do
    :crypto.strong_rand_bytes(16) |> Base.url_encode64(padding: false)
  end
end
