defmodule PoddyclipBackend.Processing.Job do
  @moduledoc """
  Ecto schema for processing jobs submitted to poddyclip-api.
  """

  use Ecto.Schema
  import Ecto.Changeset

  schema "jobs" do
    field :rust_job_id, :string
    field :filename, :string
    field :status, Ecto.Enum, values: [:queued, :processing, :completed, :failed]
    field :progress, :map, default: %{}
    field :error, :string
    field :download_url, :string
    field :result_s3_key, :string
    field :input_s3_key, :string
    field :chain, :string
    field :estimated_seconds, :integer
    field :dismissed, :boolean, default: false

    belongs_to :user, PoddyclipBackend.Accounts.User

    timestamps(type: :utc_datetime)
  end

  def changeset(job, attrs) do
    job
    |> cast(attrs, [:rust_job_id, :filename, :status, :progress, :error, :download_url, :result_s3_key, :user_id, :input_s3_key, :chain, :estimated_seconds, :dismissed])
    |> validate_required([:filename, :status, :user_id])
  end

  @doc """
  Convert a job to a map suitable for JSON serialization.
  Used by both ProcessController and JobChannel.
  """
  def to_map(job) do
    %{
      id: job.id,
      status: Atom.to_string(job.status),
      progress: job.progress || %{},
      filename: job.filename,
      download_url: job.download_url,
      error: job.error
    }
  end
end
