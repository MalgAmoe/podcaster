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
     |> assign(:s3_key, nil)
     |> assign(:filename, nil)
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
    key = Storage.input_key(user_id, entry.client_name)

    case Storage.presign_upload(key) do
      {:ok, url} ->
        meta = %{
          uploader: "S3",
          key: key,
          url: url,
          filename: entry.client_name
        }

        socket =
          socket
          |> assign(:s3_key, key)
          |> assign(:filename, entry.client_name)

        {:ok, meta, socket}

      {:error, reason} ->
        {:error, reason}
    end
  end

  @impl true
  def handle_event("select_preset", %{"preset" => preset}, socket) do
    {:noreply, assign(socket, :selected_preset, preset)}
  end

  @impl true
  def handle_event("validate", _params, socket) do
    {:noreply, socket}
  end

  @impl true
  def handle_event("cancel-upload", %{"ref" => ref}, socket) do
    {:noreply, cancel_upload(socket, :audio, ref)}
  end

  @impl true
  def handle_event("process", _params, socket) do
    user_id = socket.assigns.current_scope.user.id

    completed_entries =
      consume_uploaded_entries(socket, :audio, fn meta, entry ->
        {:ok, %{key: meta.key, filename: entry.client_name}}
      end)

    {s3_key, filename} =
      case completed_entries do
        [%{key: key, filename: name}] -> {key, name}
        [] -> {socket.assigns.s3_key, socket.assigns.filename}
      end

    if s3_key do
      opts = [chain: socket.assigns.selected_preset]

      case Processing.submit_job_from_s3(s3_key, filename, user_id, opts) do
        {:ok, job} ->
          Processing.subscribe(job.id)

          {:noreply,
           socket
           |> assign(:job, job)
           |> assign(:error, nil)}

        {:error, reason} ->
          {:noreply, assign(socket, :error, friendly_error(reason))}
      end
    else
      {:noreply, assign(socket, :error, "Please upload a file first")}
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

  @impl true
  def handle_event("reset", _params, socket) do
    if socket.assigns.job do
      Processing.cancel_job(socket.assigns.job.id)
    end

    socket =
      Enum.reduce(socket.assigns.uploads.audio.entries, socket, fn entry, sock ->
        cancel_upload(sock, :audio, entry.ref)
      end)

    {:noreply,
     socket
     |> assign(:s3_key, nil)
     |> assign(:filename, nil)
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
      <!-- Main card -->
      <div class="card bg-base-200 w-full max-w-lg border border-base-300 rounded-3xl">
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

      <!-- Steps indicator -->
      <ul class="steps steps-horizontal mt-8">
        <li class={step_class(:upload, @job)}>Feed</li>
        <li class={step_class(:process, @job)}>Munch</li>
        <li class={step_class(:download, @job)}>Enjoy</li>
      </ul>
    </div>
    """
  end

  defp upload_form(assigns) do
    upload_complete = Enum.any?(assigns.uploads.audio.entries, & &1.done?)
    assigns = assign(assigns, :upload_complete, upload_complete)

    ~H"""
    <form phx-submit="process" phx-change="validate" class="space-y-6">
      <.live_file_input upload={@uploads.audio} class="hidden" />

      <%= if @uploads.audio.entries == [] do %>
        <!-- Upload zone -->
        <label
          for={@uploads.audio.ref}
          class="border-2 border-dashed border-base-300 rounded-3xl p-12 text-center
                 hover:border-primary hover:bg-primary/5 transition-all duration-200
                 cursor-pointer group block"
          phx-drop-target={@uploads.audio.ref}
        >
          <div class="flex flex-col items-center gap-4">
            <div class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center
                        group-hover:bg-primary/20 transition-colors">
              <svg class="w-8 h-8 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
              </svg>
            </div>
            <div>
              <p class="font-medium text-base-content text-lg">Feed the cow! 🐄</p>
              <p class="text-sm text-base-content/60 mt-1">She's VERY hungry for your audio</p>
            </div>
            <p class="text-xs text-base-content/40">nom nom nom • WAV, MP3, FLAC</p>
          </div>
        </label>
      <% else %>
        <!-- File info -->
        <%= for entry <- @uploads.audio.entries do %>
          <div class="flex items-center gap-4 p-4 bg-base-300 rounded-2xl">
            <div class="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <%= if entry.done? do %>
                <svg class="w-6 h-6 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                        d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
                </svg>
              <% else %>
                <span class="loading loading-spinner loading-sm text-primary"></span>
              <% end %>
            </div>
            <div class="flex-1 min-w-0">
              <p class="font-medium truncate"><%= entry.client_name %></p>
              <%= if entry.done? do %>
                <p class="text-sm text-primary">Ready to munch! 🤤</p>
              <% else %>
                <div class="flex items-center gap-2 mt-1">
                  <progress class="progress progress-primary flex-1 h-2" value={entry.progress} max="100"></progress>
                  <span class="text-xs text-base-content/60 w-8"><%= entry.progress %>%</span>
                </div>
              <% end %>
            </div>
            <button type="button" phx-click="reset" class="btn btn-ghost btn-sm btn-circle">
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
      <div class="form-control text-center">
        <p class="font-medium mb-3">How hard should the cow chew? 🐄</p>
        <div class="flex flex-wrap gap-2 justify-center mt-2">
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
      <button type="submit" disabled={not @upload_complete} class="btn btn-primary w-full btn-lg">
        🐄 MUNCH IT!
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
        <div class="flex flex-col items-center py-8">
          <div class="radial-progress text-primary text-2xl font-bold"
               style={"--value:#{@job.progress["percent_complete"] || 0}; --size: 10rem; --thickness: 0.5rem;"}
               role="progressbar">
            <%= @job.progress["percent_complete"] || 0 %>%
          </div>

          <p class="mt-6 text-xl font-bold"><span class="inline-block animate-munch">🐄</span> *munch munch munch*</p>
          <p class="text-base-content/60 mt-2"><%= friendly_stage(@job.progress["stage"]) %></p>
          <p class="text-base-content/40 text-sm mt-1 truncate max-w-full"><%= @job.filename %></p>

          <button phx-click="reset" class="btn btn-ghost btn-sm mt-6 text-error">
            Cancel
          </button>
        </div>

      <% @job.status == :completed -> %>
        <div class="flex flex-col items-center py-8">
          <div class="w-20 h-20 rounded-full bg-primary/10 flex items-center justify-center mb-6">
            <svg class="w-10 h-10 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          </div>

          <h3 class="text-2xl font-bold">MOOO! <span class="inline-block animate-bounce-soft">🐄</span>✨</h3>
          <p class="text-base-content/60 mt-2">The cow is satisfied. Your audio is now delicious.</p>
          <p class="text-base-content/40 text-sm mt-1 truncate max-w-full"><%= @job.filename %></p>

          <div class="flex gap-3 mt-8">
            <button phx-click="download" class="btn btn-primary btn-lg gap-2">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Grab it!
            </button>
            <button phx-click="reset" class="btn btn-ghost">
              🐄 Feed me more!
            </button>
          </div>
        </div>

      <% @job.status == :failed -> %>
        <div class="flex flex-col items-center py-8">
          <div class="w-20 h-20 rounded-full bg-error/10 flex items-center justify-center mb-6">
            <svg class="w-10 h-10 text-error" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </div>

          <h3 class="text-2xl font-bold text-error">The cow choked! 🐄💀</h3>
          <p class="text-base-content/60 mt-2 text-center px-4"><%= friendly_job_error(@job.error) %></p>

          <button phx-click="reset" class="btn btn-primary mt-8">
            Feed her again
          </button>
        </div>

      <% true -> %>
        <div class="text-center py-8">
          <p>Unknown status</p>
          <button phx-click="reset" class="btn btn-ghost mt-4">Reset</button>
        </div>
    <% end %>
    """
  end

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

  defp friendly_error(:timeout), do: "Request timed out. Please try again."
  defp friendly_error(:econnrefused), do: "Cannot connect to processing server. Is it running?"
  defp friendly_error(reason) when is_binary(reason), do: reason
  defp friendly_error(_reason), do: "An unexpected error occurred. Please try again."

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

  defp friendly_stage(nil), do: "Reading audio..."
  defp friendly_stage("decoding"), do: "Reading audio..."
  defp friendly_stage("filters"), do: "Cutting rumble & hiss..."
  defp friendly_stage("input_gain"), do: "Balancing levels..."
  defp friendly_stage("analyzing_reverb"), do: "Detecting room sound..."
  defp friendly_stage("dereverb"), do: "Removing room echo..."
  defp friendly_stage("analyzing_noise"), do: "Finding background noise..."
  defp friendly_stage("denoise"), do: "Cleaning up noise..."
  defp friendly_stage("spectral_gate"), do: "Gating quiet parts..."
  defp friendly_stage("analyzing_peaks"), do: "Finding harsh tones..."
  defp friendly_stage("peak_attenuation"), do: "Smoothing harsh tones..."
  defp friendly_stage("expander"), do: "Opening up dynamics..."
  defp friendly_stage("compressor"), do: "Leveling out..."
  defp friendly_stage("analyzing_eq"), do: "Checking the tone..."
  defp friendly_stage("fixeq"), do: "Fixing muddy spots..."
  defp friendly_stage("deesser"), do: "Taming the S's..."
  defp friendly_stage("saturation"), do: "Adding warmth..."
  defp friendly_stage("buttercomp"), do: "Gluing it together..."
  defp friendly_stage("analyzing_enhance"), do: "Optimizing presence..."
  defp friendly_stage("enhanceeq"), do: "Brightening up..."
  defp friendly_stage("radio"), do: "Broadcast polish..."
  defp friendly_stage("tape"), do: "Adding analog feel..."
  defp friendly_stage("analyzing_levels"), do: "Measuring loudness..."
  defp friendly_stage("output"), do: "Final limiting..."
  defp friendly_stage("encoding"), do: "Saving your file..."
  defp friendly_stage("completed"), do: "Done! 🐄✨"
  defp friendly_stage(other), do: other

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
