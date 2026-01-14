defmodule PoddyclipBackendWeb.ProcessLive do
  use PoddyclipBackendWeb, :live_view

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Processing.Client
  alias PoddyclipBackend.Storage

  @impl true
  def mount(_params, _session, socket) do
    # Fetch available presets
    presets =
      case Client.list_presets() do
        {:ok, %{"chain_presets" => presets}} ->
          Enum.map(presets, fn %{"name" => name} -> name end)

        _ ->
          ["podcast", "broadcast", "gentle"]
      end

    {:ok,
     socket
     |> assign(:presets, presets)
     |> assign(:selected_preset, "podcast")
     |> assign(:job, nil)
     |> assign(:error, nil)
     |> assign(:uploaded_file, nil)
     |> allow_upload(:audio,
       accept: ~w(audio/*),
       max_file_size: 100_000_000,
       external: &presign_upload/2,
       auto_upload: true
     )}
  end

  # Generate presigned URL for direct S3 upload
  defp presign_upload(entry, socket) do
    user_id = socket.assigns.current_scope.user.id
    key = Storage.input_key(user_id)

    case Storage.presign_upload(user_id, entry.client_name) do
      {:ok, %{upload_url: url}} ->
        meta = %{
          uploader: "S3",
          key: key,
          url: url,
          filename: entry.client_name
        }
        {:ok, meta, socket}

      {:error, reason} ->
        {:error, reason}
    end
  end

  # Preset selection
  @impl true
  def handle_event("select_preset", %{"preset" => preset}, socket) do
    {:noreply, assign(socket, :selected_preset, preset)}
  end

  # Validate upload
  @impl true
  def handle_event("validate", _params, socket) do
    {:noreply, socket}
  end

  # Cancel upload
  @impl true
  def handle_event("cancel-upload", %{"ref" => ref}, socket) do
    {:noreply, cancel_upload(socket, :audio, ref)}
  end

  # Process - file already uploaded to S3, just start the job
  @impl true
  def handle_event("process", _params, socket) do
    user_id = socket.assigns.current_scope.user.id

    # Get the uploaded file info from completed entries
    completed_entries =
      consume_uploaded_entries(socket, :audio, fn meta, entry ->
        {:ok, %{key: meta.key, filename: entry.client_name}}
      end)

    case completed_entries do
      [%{key: input_key, filename: filename}] ->
        opts = [chain: socket.assigns.selected_preset]

        case Processing.submit_job_from_s3(input_key, filename, user_id, opts) do
          {:ok, job} ->
            Processing.subscribe(job.id)

            {:noreply,
             socket
             |> assign(:job, job)
             |> assign(:error, nil)}

          {:error, reason} ->
            {:noreply, assign(socket, :error, friendly_error(reason))}
        end

      [] ->
        {:noreply, assign(socket, :error, "Please select a file first")}
    end
  end

  @impl true
  def handle_event("download", _params, socket) do
    job = socket.assigns.job

    if job && job.status == :completed && job.download_url do
      {:noreply, redirect(socket, external: job.download_url)}
    else
      {:noreply, assign(socket, :error, "Download not available")}
    end
  end

  # Reset - allow starting over
  @impl true
  def handle_event("reset", _params, socket) do
    # Cancel job if in progress
    if socket.assigns.job do
      Processing.cancel_job(socket.assigns.job.id)
    end

    {:noreply,
     socket
     |> assign(:pending_file, nil)
     |> assign(:job, nil)
     |> assign(:error, nil)}
  end

  @impl true
  def handle_info({:job_updated, job}, socket) do
    {:noreply, assign(socket, :job, job)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-[calc(100vh-4rem)] flex flex-col items-center justify-center p-4">
      <!-- Steps indicator -->
      <ul class="steps steps-horizontal mb-8">
        <li class={step_class(:upload, @job)}>Upload</li>
        <li class={step_class(:process, @job)}>Process</li>
        <li class={step_class(:download, @job)}>Download</li>
      </ul>

      <!-- Main card -->
      <div class="card card-bordered bg-base-100 w-full max-w-lg shadow-lg">
        <div class="card-body">
          <%= if @error do %>
            <div role="alert" class="alert alert-error mb-4">
              <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 shrink-0 stroke-current" fill="none" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span><%= @error %></span>
            </div>
          <% end %>

          <%= if @job do %>
            <.job_status job={@job} />
          <% else %>
            <.upload_form uploads={@uploads} presets={@presets} selected_preset={@selected_preset} />
          <% end %>
        </div>
      </div>
    </div>
    """
  end

  defp upload_form(assigns) do
    ~H"""
    <form phx-submit="process" phx-change="validate" class="space-y-6">
      <!-- Hidden file input - always rendered -->
      <.live_file_input upload={@uploads.audio} class="hidden" />

      <!-- Upload zone -->
      <%= if @uploads.audio.entries == [] do %>
        <label
          for={@uploads.audio.ref}
          class="border-2 border-dashed border-base-300 rounded-2xl p-12 text-center
                 hover:border-primary hover:bg-primary/5 transition-all duration-200
                 cursor-pointer group block"
          phx-drop-target={@uploads.audio.ref}
        >
          <div class="flex flex-col items-center gap-4">
            <!-- Upload icon in circle -->
            <div class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center
                        group-hover:bg-primary/20 transition-colors">
              <svg class="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
              </svg>
            </div>
            <div>
              <p class="font-medium text-base-content">Drop your audio file here</p>
              <p class="text-sm text-base-content/60 mt-1">or click to browse</p>
            </div>
            <p class="text-xs text-base-content/40">WAV, MP3, FLAC • Max 100MB</p>
          </div>
        </label>
      <% else %>
        <!-- File selected state -->
        <%= for entry <- @uploads.audio.entries do %>
          <div class="flex items-center gap-4 p-4 bg-base-200 rounded-xl">
            <div class="w-12 h-12 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
              <svg class="w-6 h-6 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <p class="font-medium truncate"><%= entry.client_name %></p>
              <%= if entry.done? do %>
                <p class="text-sm text-success">Ready to process</p>
              <% else %>
                <div class="flex items-center gap-2 mt-1">
                  <progress class="progress progress-primary flex-1 h-2" value={entry.progress} max="100"></progress>
                  <span class="text-xs text-base-content/60 w-8"><%= entry.progress %>%</span>
                </div>
              <% end %>
            </div>
            <button type="button" phx-click="cancel-upload" phx-value-ref={entry.ref}
                    class="btn btn-ghost btn-sm btn-circle">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <%= for err <- upload_errors(@uploads.audio, entry) do %>
            <p class="text-error text-sm"><%= error_to_string(err) %></p>
          <% end %>
        <% end %>
      <% end %>

      <!-- Preset selection -->
      <div class="form-control">
        <label class="label">
          <span class="label-text font-medium">Processing Style</span>
        </label>
        <div class="flex flex-wrap gap-2">
          <%= for preset <- @presets do %>
            <button
              type="button"
              phx-click="select_preset"
              phx-value-preset={preset}
              class={"btn btn-sm " <> if preset == @selected_preset, do: "btn-primary", else: "btn-outline"}
            >
              <%= String.capitalize(preset) %>
            </button>
          <% end %>
        </div>
      </div>

      <!-- Submit button -->
      <button
        type="submit"
        disabled={@uploads.audio.entries == [] or not Enum.all?(@uploads.audio.entries, & &1.done?)}
        class="btn btn-primary w-full btn-lg"
      >
        <%= if Enum.any?(@uploads.audio.entries, & not &1.done?) do %>
          <span class="loading loading-spinner loading-sm"></span>
          Uploading...
        <% else %>
          Process Audio
        <% end %>
      </button>
    </form>
    """
  end

  defp error_to_string(:too_large), do: "File is too large (max 100MB)"
  defp error_to_string(:not_accepted), do: "File type not accepted"
  defp error_to_string(:too_many_files), do: "Too many files"
  defp error_to_string(err), do: "Error: #{inspect(err)}"

  defp job_status(assigns) do
    ~H"""
    <%= cond do %>
      <% @job.status in [:queued, :processing] -> %>
        <!-- Processing state with radial progress -->
        <div class="flex flex-col items-center py-8">
          <div class="radial-progress text-primary text-2xl font-bold"
               style={"--value:#{@job.progress["percent_complete"] || 0}; --size: 10rem; --thickness: 0.5rem;"}
               role="progressbar">
            <%= @job.progress["percent_complete"] || 0 %>%
          </div>

          <p class="mt-6 text-lg font-medium"><%= @job.progress["stage"] || "Starting..." %></p>
          <p class="text-base-content/60 text-sm mt-1 truncate max-w-full"><%= @job.filename %></p>

          <button phx-click="reset" class="btn btn-ghost btn-sm mt-6 text-error">
            Cancel
          </button>
        </div>

      <% @job.status == :completed -> %>
        <!-- Completed state -->
        <div class="flex flex-col items-center py-8">
          <div class="w-20 h-20 rounded-full bg-success/10 flex items-center justify-center mb-6">
            <svg class="w-10 h-10 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          </div>

          <h3 class="text-xl font-bold">Processing Complete!</h3>
          <p class="text-base-content/60 mt-2 truncate max-w-full"><%= @job.filename %></p>

          <div class="flex gap-3 mt-8">
            <button phx-click="download" class="btn btn-success btn-lg gap-2">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Download
            </button>
            <button phx-click="reset" class="btn btn-ghost">
              Process Another
            </button>
          </div>
        </div>

      <% @job.status == :failed -> %>
        <!-- Failed state -->
        <div class="flex flex-col items-center py-8">
          <div class="w-20 h-20 rounded-full bg-error/10 flex items-center justify-center mb-6">
            <svg class="w-10 h-10 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>

          <h3 class="text-xl font-bold text-error">Processing Failed</h3>
          <p class="text-base-content/60 mt-2 text-center px-4"><%= friendly_job_error(@job.error) %></p>

          <button phx-click="reset" class="btn btn-primary mt-8">
            Try Again
          </button>
        </div>

      <% true -> %>
        <!-- Unknown state -->
        <div class="text-center py-8">
          <p>Unknown status</p>
          <button phx-click="reset" class="btn btn-ghost mt-4">Reset</button>
        </div>
    <% end %>
    """
  end

  # Step indicator helpers
  defp step_class(step, job) do
    current = current_step(job)
    cond do
      step_order(step) < step_order(current) -> "step step-primary"
      step_order(step) == step_order(current) -> "step step-primary"
      true -> "step"
    end
  end

  defp current_step(nil), do: :upload
  defp current_step(%{status: :completed}), do: :download
  defp current_step(%{status: :failed}), do: :process
  defp current_step(_job), do: :process

  defp step_order(:upload), do: 1
  defp step_order(:process), do: 2
  defp step_order(:download), do: 3

  # User-friendly error messages
  defp friendly_error({:http_error, _status, %{"error" => %{"type" => type, "message" => message}}}) do
    case type do
      "file_too_large" -> "File is too large. #{message}"
      "unsupported_format" -> "This audio format isn't supported. Try WAV or MP3."
      "server_busy" -> "Server is busy. Please try again in a moment."
      "chain_not_found" -> "Selected preset is not available."
      "processing_error" -> "Failed to process audio. #{simplify_processing_error(message)}"
      "internal_error" -> "Something went wrong. Please try again."
      _ -> message
    end
  end

  defp friendly_error({:http_error, status, _body}) when status >= 500 do
    "Server error. Please try again later."
  end

  defp friendly_error({:http_error, _status, _body}) do
    "Request failed. Please try again."
  end

  defp friendly_error(:timeout) do
    "Request timed out. Please try again."
  end

  defp friendly_error(:econnrefused) do
    "Cannot connect to processing server. Is it running?"
  end

  defp friendly_error(reason) when is_binary(reason) do
    reason
  end

  defp friendly_error(_reason) do
    "An unexpected error occurred. Please try again."
  end

  # Simplify technical processing errors
  defp simplify_processing_error(message) do
    cond do
      String.contains?(message, "probe") or String.contains?(message, "codec") ->
        "The audio format could not be read."
      String.contains?(message, "decode") ->
        "Failed to decode the audio file."
      String.contains?(message, "timeout") ->
        "Processing took too long."
      String.contains?(message, "S3") or String.contains?(message, "storage") ->
        "Storage error occurred."
      true ->
        "Please try a different file."
    end
  end

  # Simplify job error messages from Rust
  defp friendly_job_error(nil), do: "Unknown error"
  defp friendly_job_error(error) when is_binary(error) do
    cond do
      String.contains?(error, "probe") or String.contains?(error, "Unsupported") ->
        "Audio format not supported. Try converting to WAV or MP3."
      String.contains?(error, "No audio track") ->
        "No audio found in file."
      String.contains?(error, "decode") or String.contains?(error, "Decoding") ->
        "Could not read the audio file. It may be corrupted."
      String.contains?(error, "timeout") or String.contains?(error, "timed out") ->
        "Processing took too long. Try a shorter file."
      String.contains?(error, "S3") or String.contains?(error, "download") ->
        "Could not access the file. Please re-upload."
      String.contains?(error, "encode") or String.contains?(error, "MP3") ->
        "Failed to create output file."
      true ->
        error
    end
  end
end
