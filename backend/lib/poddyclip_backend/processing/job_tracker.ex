defmodule PoddyclipBackend.Processing.JobTracker do
  @moduledoc """
  GenServer that tracks processing jobs and polls for status updates.
  Broadcasts updates via PubSub for real-time client notifications.
  """

  use GenServer
  require Logger

  alias PoddyclipBackend.Processing.{Client, Job}

  @poll_interval 1_000  # 1 second

  # Client API

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @doc """
  Submit a new job for processing.
  Returns {:ok, job} on success.
  """
  def submit_job(audio_binary, filename, opts \\ []) do
    GenServer.call(__MODULE__, {:submit_job, audio_binary, filename, opts}, 30_000)
  end

  @doc """
  Get the current status of a job.
  """
  def get_job(job_id) do
    GenServer.call(__MODULE__, {:get_job, job_id})
  end

  @doc """
  List all jobs, optionally filtered by user_id.
  """
  def list_jobs(opts \\ []) do
    GenServer.call(__MODULE__, {:list_jobs, opts})
  end

  @doc """
  Cancel and delete a job.
  """
  def cancel_job(job_id) do
    GenServer.call(__MODULE__, {:cancel_job, job_id})
  end

  @doc """
  Subscribe to job updates for a specific job.
  """
  def subscribe(job_id) do
    Phoenix.PubSub.subscribe(PoddyclipBackend.PubSub, "job:#{job_id}")
  end

  @doc """
  Subscribe to all job updates.
  """
  def subscribe_all do
    Phoenix.PubSub.subscribe(PoddyclipBackend.PubSub, "jobs")
  end

  # Server Callbacks

  @impl true
  def init(_opts) do
    state = %{
      jobs: %{},  # job_id => Job
      rust_to_local: %{}  # rust_job_id => job_id
    }
    {:ok, state}
  end

  @impl true
  def handle_call({:submit_job, audio_binary, filename, opts}, _from, state) do
    job = Job.new(filename, opts)

    # Submit to rust service
    case Client.process_audio(audio_binary, filename, opts) do
      {:ok, %{"job_id" => rust_job_id}} ->
        job = %{job | rust_job_id: rust_job_id, status: :processing}

        new_state = %{state |
          jobs: Map.put(state.jobs, job.id, job),
          rust_to_local: Map.put(state.rust_to_local, rust_job_id, job.id)
        }

        # Start polling for this job
        schedule_poll(rust_job_id)

        broadcast_update(job)
        {:reply, {:ok, job}, new_state}

      {:error, reason} ->
        Logger.error("Failed to submit job: #{inspect(reason)}")
        {:reply, {:error, reason}, state}
    end
  end

  @impl true
  def handle_call({:get_job, job_id}, _from, state) do
    case Map.get(state.jobs, job_id) do
      nil -> {:reply, {:error, :not_found}, state}
      job -> {:reply, {:ok, job}, state}
    end
  end

  @impl true
  def handle_call({:list_jobs, opts}, _from, state) do
    jobs = Map.values(state.jobs)

    jobs =
      if user_id = Keyword.get(opts, :user_id) do
        Enum.filter(jobs, &(&1.user_id == user_id))
      else
        jobs
      end

    # Sort by created_at descending
    jobs = Enum.sort_by(jobs, & &1.created_at, {:desc, DateTime})

    {:reply, {:ok, jobs}, state}
  end

  @impl true
  def handle_call({:cancel_job, job_id}, _from, state) do
    case Map.get(state.jobs, job_id) do
      nil ->
        {:reply, {:error, :not_found}, state}

      job ->
        # Try to cancel in rust service
        if job.rust_job_id do
          Client.delete_job(job.rust_job_id)
        end

        new_state = %{state |
          jobs: Map.delete(state.jobs, job_id),
          rust_to_local: Map.delete(state.rust_to_local, job.rust_job_id)
        }

        {:reply, :ok, new_state}
    end
  end

  @impl true
  def handle_info({:poll, rust_job_id}, state) do
    case Map.get(state.rust_to_local, rust_job_id) do
      nil ->
        # Job was deleted, stop polling
        {:noreply, state}

      job_id ->
        case poll_job_status(rust_job_id, job_id, state) do
          {:continue, new_state} ->
            schedule_poll(rust_job_id)
            {:noreply, new_state}

          {:done, new_state} ->
            {:noreply, new_state}
        end
    end
  end

  # Private functions

  defp schedule_poll(rust_job_id) do
    Process.send_after(self(), {:poll, rust_job_id}, @poll_interval)
  end

  defp poll_job_status(rust_job_id, job_id, state) do
    case Client.get_job_status(rust_job_id) do
      {:ok, rust_status} ->
        job = Map.get(state.jobs, job_id)
        updated_job = Job.update_from_rust_status(job, rust_status)

        new_state = %{state | jobs: Map.put(state.jobs, job_id, updated_job)}
        broadcast_update(updated_job)

        case updated_job.status do
          status when status in [:completed, :failed] ->
            {:done, new_state}

          _ ->
            {:continue, new_state}
        end

      {:error, :not_found} ->
        # Job was deleted from rust service
        job = Map.get(state.jobs, job_id)
        failed_job = %{job | status: :failed, error: "Job not found in processing service"}

        new_state = %{state | jobs: Map.put(state.jobs, job_id, failed_job)}
        broadcast_update(failed_job)
        {:done, new_state}

      {:error, reason} ->
        Logger.warning("Failed to poll job #{rust_job_id}: #{inspect(reason)}")
        {:continue, state}
    end
  end

  defp broadcast_update(job) do
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "job:#{job.id}", {:job_updated, job})
    Phoenix.PubSub.broadcast(PoddyclipBackend.PubSub, "jobs", {:job_updated, job})
  end
end
