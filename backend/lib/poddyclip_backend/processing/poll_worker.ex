defmodule PoddyclipBackend.Processing.PollWorker do
  @moduledoc """
  Oban worker that polls the Rust API for job status updates.
  Reschedules itself until the job completes or fails.
  """

  use Oban.Worker, queue: :processing, max_attempts: 100

  alias PoddyclipBackend.Processing.{Job, Client}
  alias PoddyclipBackend.Repo

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"job_id" => job_id}}) do
    job = Repo.get!(Job, job_id)

    case Client.get_job_status(job.rust_job_id) do
      {:ok, rust_status} ->
        updated_job =
          job
          |> Job.changeset(parse_rust_status(rust_status))
          |> Repo.update!()

        broadcast_update(updated_job)

        if rust_status["status"] in ["completed", "failed"] do
          :ok
        else
          # Reschedule in 1 second
          {:snooze, 1}
        end

      {:error, :not_found} ->
        updated_job =
          job
          |> Job.changeset(%{status: :failed, error: "Job not found in processing service"})
          |> Repo.update!()

        broadcast_update(updated_job)
        :ok

      {:error, _reason} ->
        # Retry after 2 seconds on transient errors
        {:snooze, 2}
    end
  end

  defp parse_rust_status(status) do
    %{
      status: parse_status(status["status"]),
      progress: status["progress"] || %{},
      error: status["error"],
      download_url: status["download_url"]
    }
  end

  defp parse_status("queued"), do: :queued
  defp parse_status("processing"), do: :processing
  defp parse_status("completed"), do: :completed
  defp parse_status("failed"), do: :failed
  defp parse_status(_), do: :processing

  defp broadcast_update(job) do
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "job:#{job.id}", {:job_updated, job})
  end
end
