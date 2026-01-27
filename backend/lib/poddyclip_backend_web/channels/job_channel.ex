defmodule PoddyclipBackendWeb.JobChannel do
  use Phoenix.Channel

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Storage

  @doc """
  Join a job channel. Verifies the user owns the job.
  Topic format: "job:{job_id}"
  """
  @impl true
  def join("job:" <> job_id, _params, socket) do
    case Processing.get_job(job_id) do
      nil ->
        {:error, %{reason: "not_found"}}

      job when job.user_id != socket.assigns.current_user_id ->
        {:error, %{reason: "unauthorized"}}

      job ->
        # Subscribe to PubSub for this job (reuses existing mechanism)
        Processing.subscribe(job.id)

        # Send current job state immediately
        {:ok, %{job: serialize_job(job)}, socket}
    end
  end

  @doc """
  Handle PubSub broadcasts and forward to channel.
  """
  @impl true
  def handle_info({:job_updated, job}, socket) do
    push(socket, "job_updated", %{job: serialize_job(job)})
    {:noreply, socket}
  end

  defp serialize_job(job) do
    Job.to_map(job)
    |> Map.put(:original_url, presign_key(job.input_s3_key))
  end

  defp presign_key(nil), do: nil
  defp presign_key(key) do
    case Storage.presign_download(key) do
      {:ok, url} -> url
      _ -> nil
    end
  end
end
