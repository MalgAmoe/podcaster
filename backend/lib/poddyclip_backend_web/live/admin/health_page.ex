defmodule PoddyclipBackendWeb.Live.Admin.HealthPage do
  @moduledoc """
  Custom LiveDashboard page for system health monitoring.

  Shows:
  - Service health indicators (database, S3, Rust API)
  - Oban queue status
  - User statistics
  """

  use Phoenix.LiveDashboard.PageBuilder
  alias PoddyclipBackend.Admin

  @impl true
  def menu_link(_, _) do
    {:ok, "Health"}
  end

  @impl true
  def mount(_params, _session, socket) do
    {:ok, assign(socket, health: nil, user_stats: nil)}
  end

  @impl true
  def handle_refresh(socket) do
    health = Admin.health_check()
    user_stats = Admin.user_stats()

    {:noreply, assign(socket, health: health, user_stats: user_stats)}
  end

  @impl true
  def render(assigns) do
    health = assigns[:health] || Admin.health_check()
    user_stats = assigns[:user_stats] || Admin.user_stats()

    assigns = assign(assigns, health: health, user_stats: user_stats)

    ~H"""
    <.row>
      <:col>
        <.card title="System Health" inner_title="Overall Status">
          <span class={status_class(@health.status)}>
            <%= String.upcase(@health.status) %>
          </span>
        </.card>
      </:col>
      <:col>
        <.card title="User Stats" inner_title="Total Users">
          <%= @user_stats.total %>
        </.card>
      </:col>
      <:col>
        <.card title="Seconds" inner_title="Available">
          <%= @user_stats.total_seconds_available %>
        </.card>
      </:col>
    </.row>

    <.fields_card
      title="Service Checks"
      inner_title="Health Status"
      fields={service_check_fields(@health.checks)}
    />

    <.fields_card
      :if={@health.checks.oban.status == "ok"}
      title="Oban Queues"
      inner_title="Queue Status"
      fields={oban_queue_fields(@health.checks.oban.queues)}
    />

    <.fields_card
      title="Users by Plan"
      inner_title="Distribution"
      fields={plan_fields(@user_stats.by_plan)}
    />

    <.fields_card
      title="Users by Subscription"
      inner_title="Status"
      fields={subscription_fields(@user_stats.by_subscription)}
    />

    <.card title="Timestamp" inner_title="Last Updated">
      <%= Calendar.strftime(@health.timestamp, "%Y-%m-%d %H:%M:%S UTC") %>
    </.card>
    """
  end

  defp status_class("healthy"), do: "text-success"
  defp status_class("degraded"), do: "text-warning"
  defp status_class("unhealthy"), do: "text-danger"
  defp status_class(_), do: ""

  defp service_check_fields(checks) do
    [
      {"Database", "#{status_badge(checks.database.status)} #{service_details(checks.database)}"},
      {"S3 Storage", "#{status_badge(checks.s3.status)} #{service_details(checks.s3)}"},
      {"Rust API", "#{status_badge(checks.rust_api.status)} #{service_details(checks.rust_api)}"},
      {"Oban", status_badge(checks.oban.status)}
    ]
  end

  defp status_badge("ok"), do: "[OK]"
  defp status_badge("error"), do: "[ERROR]"
  defp status_badge("disabled"), do: "[DISABLED]"
  defp status_badge(_), do: "[UNKNOWN]"

  defp service_details(check) do
    cond do
      Map.has_key?(check, :latency_ms) -> "(#{check.latency_ms}ms)"
      Map.has_key?(check, :error) -> "(#{truncate(check.error, 40)})"
      Map.has_key?(check, :message) -> "(#{check.message})"
      true -> ""
    end
  end

  defp oban_queue_fields(queues) do
    Enum.map(queues, fn {name, info} ->
      {to_string(name), "Limit: #{info.limit} | Executing: #{info.executing} | Available: #{info.available}"}
    end)
  end

  defp plan_fields(by_plan) when map_size(by_plan) == 0, do: [{"Status", "No users yet"}]
  defp plan_fields(by_plan) do
    Enum.map(by_plan, fn {plan, count} ->
      {plan || "none", "#{count} users"}
    end)
  end

  defp subscription_fields(by_sub) when map_size(by_sub) == 0, do: [{"Status", "No users yet"}]
  defp subscription_fields(by_sub) do
    Enum.map(by_sub, fn {status, count} ->
      {status, "#{count} users"}
    end)
  end

  defp truncate(nil, _), do: ""
  defp truncate(str, max) when byte_size(str) <= max, do: str
  defp truncate(str, max), do: String.slice(str, 0, max - 3) <> "..."
end
