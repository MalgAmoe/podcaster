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
     |> assign(:pending_file, nil)
     |> allow_upload(:audio,
       accept: ~w(audio/*),
       max_file_size: 100_000_000,
       progress: &handle_progress/3,
       auto_upload: true
     )}
  end

  # Track upload progress
  defp handle_progress(:audio, entry, socket) do
    if entry.done? do
      {:noreply, socket}
    else
      {:noreply, socket}
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

  # Process - upload to S3 then start job
  @impl true
  def handle_event("process", _params, socket) do
    user_id = socket.assigns.current_scope.user.id

    # Consume uploaded file and upload to S3
    # Note: consume_uploaded_entries requires {:ok, _} return, so we wrap errors
    uploaded_files =
      consume_uploaded_entries(socket, :audio, fn %{path: path}, entry ->
        filename = entry.client_name
        content = File.read!(path)

        case Storage.upload(user_id, filename, content) do
          {:ok, key} -> {:ok, {:ok, filename, key}}
          {:error, reason} -> {:ok, {:error, reason}}
        end
      end)

    case uploaded_files do
      [{:ok, filename, input_key}] ->
        opts = [chain: socket.assigns.selected_preset]

        case Processing.submit_job_from_s3(input_key, filename, user_id, opts) do
          {:ok, job} ->
            Processing.subscribe(job.id)

            {:noreply,
             socket
             |> assign(:job, job)
             |> assign(:error, nil)}

          {:error, reason} ->
            {:noreply, assign(socket, :error, "Failed: #{inspect(reason)}")}
        end

      [{:error, reason}] ->
        {:noreply, assign(socket, :error, "S3 upload failed: #{inspect(reason)}")}

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
    <div class="max-w-2xl mx-auto p-6">
      <h1 class="text-3xl font-bold mb-6">Poddyclip Audio Processor</h1>

      <%= if @error do %>
        <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">
          <%= @error %>
        </div>
      <% end %>

      <%= if @job do %>
        <.job_status job={@job} />
      <% else %>
        <.upload_form uploads={@uploads} presets={@presets} selected_preset={@selected_preset} />
      <% end %>
    </div>
    """
  end

  defp upload_form(assigns) do
    ~H"""
    <form phx-submit="process" phx-change="validate" class="space-y-6">
      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">
          Select Audio File
        </label>
        <div
          class="border-2 border-dashed border-gray-300 rounded-lg p-6 text-center hover:border-gray-400 transition-colors"
          phx-drop-target={@uploads.audio.ref}
        >
          <.live_file_input upload={@uploads.audio} class="hidden" />
          <label for={@uploads.audio.ref} class="cursor-pointer">
            <div class="text-gray-500">
              <svg
                class="mx-auto h-12 w-12 text-gray-400"
                stroke="currentColor"
                fill="none"
                viewBox="0 0 48 48"
              >
                <path
                  d="M28 8H12a4 4 0 00-4 4v20m32-12v8m0 0v8a4 4 0 01-4 4H12a4 4 0 01-4-4v-4m32-4l-3.172-3.172a4 4 0 00-5.656 0L28 28M8 32l9.172-9.172a4 4 0 015.656 0L28 28m0 0l4 4m4-24h8m-4-4v8m-12 4h.02"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              <p class="mt-1">Click or drag to select audio file</p>
              <p class="text-xs text-gray-400 mt-1">WAV, MP3, FLAC up to 100MB</p>
            </div>
          </label>
        </div>

        <%= for entry <- @uploads.audio.entries do %>
          <div class="mt-4 space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium"><%= entry.client_name %></span>
              <button type="button" phx-click="cancel-upload" phx-value-ref={entry.ref} class="text-red-500 text-sm">
                Cancel
              </button>
            </div>
            <div class="w-full bg-gray-200 rounded-full h-2">
              <div
                class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                style={"width: #{entry.progress}%"}
              />
            </div>
            <p class="text-xs text-gray-500"><%= entry.progress %>%</p>

            <%= for err <- upload_errors(@uploads.audio, entry) do %>
              <p class="text-red-500 text-sm"><%= error_to_string(err) %></p>
            <% end %>
          </div>
        <% end %>
      </div>

      <div>
        <label class="block text-sm font-medium text-gray-700 mb-2">
          Select Processing Preset
        </label>
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
          <%= for preset <- @presets do %>
            <button
              type="button"
              phx-click="select_preset"
              phx-value-preset={preset}
              class={preset_button_class(preset, @selected_preset)}
            >
              <%= String.capitalize(preset) %>
            </button>
          <% end %>
        </div>
      </div>

      <button
        type="submit"
        disabled={@uploads.audio.entries == [] or not Enum.all?(@uploads.audio.entries, & &1.done?)}
        class="w-full bg-blue-600 text-white py-3 px-4 rounded-lg font-medium hover:bg-blue-700 transition-colors disabled:bg-gray-300 disabled:cursor-not-allowed"
      >
        <%= if Enum.any?(@uploads.audio.entries, & not &1.done?) do %>
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

  defp preset_button_class(preset, selected_preset) do
    base = "px-4 py-2 rounded border text-sm font-medium transition-colors "

    if preset == selected_preset do
      base <> "bg-blue-600 text-white border-blue-600"
    else
      base <> "bg-white text-gray-700 border-gray-300 hover:bg-gray-50"
    end
  end

  defp job_status(assigns) do
    ~H"""
    <div class="space-y-6">
      <div class="bg-gray-50 rounded-lg p-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-medium"><%= @job.filename %></h2>
          <span class={"px-2 py-1 rounded text-sm font-medium " <> status_color(@job.status)}>
            <%= status_text(@job.status) %>
          </span>
        </div>

        <%= if @job.status in [:queued, :processing] do %>
          <div class="mb-4">
            <div class="flex justify-between text-sm text-gray-600 mb-1">
              <span><%= @job.progress["stage"] || "Waiting..." %></span>
              <span><%= @job.progress["percent_complete"] || 0 %>%</span>
            </div>
            <div class="w-full bg-gray-200 rounded-full h-2">
              <div
                class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                style={"width: #{@job.progress["percent_complete"] || 0}%"}
              />
            </div>
          </div>
        <% end %>

        <%= if @job.status == :failed && @job.error do %>
          <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
            <%= @job.error %>
          </div>
        <% end %>

        <div class="flex gap-3 mt-4">
          <%= if @job.status == :completed do %>
            <button
              phx-click="download"
              class="flex-1 bg-green-600 text-white py-2 px-4 rounded font-medium hover:bg-green-700 transition-colors"
            >
              Download Result
            </button>
          <% end %>

          <button
            phx-click="reset"
            class={"py-2 px-4 rounded font-medium transition-colors " <>
              if @job.status == :completed do
                "flex-1 bg-gray-200 text-gray-700 hover:bg-gray-300"
              else
                "flex-1 bg-red-100 text-red-700 hover:bg-red-200"
              end}
          >
            <%= if @job.status == :completed, do: "Process Another", else: "Cancel" %>
          </button>
        </div>
      </div>
    </div>
    """
  end

  defp status_color(:queued), do: "bg-yellow-100 text-yellow-800"
  defp status_color(:processing), do: "bg-blue-100 text-blue-800"
  defp status_color(:completed), do: "bg-green-100 text-green-800"
  defp status_color(:failed), do: "bg-red-100 text-red-800"
  defp status_color(_), do: "bg-gray-100 text-gray-800"

  defp status_text(:queued), do: "Queued"
  defp status_text(:processing), do: "Processing"
  defp status_text(:completed), do: "Completed"
  defp status_text(:failed), do: "Failed"
  defp status_text(_), do: "Unknown"
end
