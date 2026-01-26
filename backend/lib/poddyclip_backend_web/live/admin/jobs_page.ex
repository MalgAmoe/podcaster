defmodule PoddyclipBackendWeb.Live.Admin.JobsPage do
  @moduledoc """
  Custom LiveDashboard page for job statistics and monitoring.

  Shows:
  - Job status counts (queued, processing, completed, failed)
  - Recent failures with error messages
  - Active jobs in progress
  """

  use Phoenix.LiveDashboard.PageBuilder
  alias PoddyclipBackend.Admin

  @impl true
  def menu_link(_, _) do
    {:ok, "Jobs"}
  end

  @impl true
  def mount(_params, _session, socket) do
    {:ok, assign(socket, stats: nil, errors: nil, active: nil)}
  end

  @impl true
  def handle_refresh(socket) do
    stats = Admin.job_stats()
    errors = Admin.recent_errors(10)
    active = Admin.active_jobs()

    {:noreply, assign(socket, stats: stats, errors: errors, active: active)}
  end

  @impl true
  def render(assigns) do
    stats = assigns[:stats] || Admin.job_stats()
    errors = assigns[:errors] || Admin.recent_errors(10)
    active = assigns[:active] || Admin.active_jobs()

    assigns = assign(assigns, stats: stats, errors: errors, active: active)

    ~H"""
    <.row>
      <:col>
        <.card title="Queue Status" inner_title="Queued">
          <span class="text-primary"><%= @stats.current.queued %></span>
        </.card>
      </:col>
      <:col>
        <.card title="Processing" inner_title="In Progress">
          <span class="text-warning"><%= @stats.current.processing %></span>
        </.card>
      </:col>
      <:col>
        <.card title="Completed (24h)" inner_title="Success">
          <span class="text-success"><%= @stats.last_24h.completed %></span>
        </.card>
      </:col>
    </.row>

    <.row>
      <:col>
        <.card title="Failed (24h)" inner_title="Errors">
          <span class="text-danger"><%= @stats.last_24h.failed %></span>
        </.card>
      </:col>
      <:col>
        <.card title="Success Rate (24h)" inner_title="Rate">
          <span class={success_rate_class(@stats.last_24h.success_rate)}>
            <%= @stats.last_24h.success_rate %>%
          </span>
        </.card>
      </:col>
      <:col>
        <.card title="Total Active" inner_title="Jobs">
          <%= length(@active) %>
        </.card>
      </:col>
    </.row>

    <.fields_card
      title={"Active Jobs (#{length(@active)})"}
      inner_title="Currently Processing"
      fields={active_job_fields(@active)}
    />

    <.fields_card
      title={"Recent Errors (#{@errors.total_24h} in 24h)"}
      inner_title="Failed Jobs"
      fields={error_fields(@errors.errors)}
    />
    """
  end

  defp success_rate_class(rate) when rate >= 95.0, do: "text-success"
  defp success_rate_class(rate) when rate >= 80.0, do: "text-warning"
  defp success_rate_class(_), do: "text-danger"

  defp active_job_fields([]), do: [{"Status", "No active jobs"}]
  defp active_job_fields(jobs) do
    Enum.map(jobs, fn job ->
      {"Job ##{job.job_id}", "#{job.filename} (#{job.status}) - User: #{job.user_id}"}
    end)
  end

  defp error_fields([]), do: [{"Status", "No errors in the last 24 hours"}]
  defp error_fields(errors) do
    Enum.map(errors, fn error ->
      label = "Job ##{error.job_id}"
      value = "#{truncate(error.filename, 30)} - #{truncate(error.error || "Unknown error", 60)} | User: #{error.user_id} | #{format_datetime(error.failed_at)}"
      {label, value}
    end)
  end

  defp truncate(nil, _), do: ""
  defp truncate(str, max) when byte_size(str) <= max, do: str
  defp truncate(str, max), do: String.slice(str, 0, max - 3) <> "..."

  defp format_datetime(nil), do: "-"
  defp format_datetime(dt), do: Calendar.strftime(dt, "%Y-%m-%d %H:%M")
end
