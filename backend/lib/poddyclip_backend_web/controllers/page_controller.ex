defmodule PoddyclipBackendWeb.PageController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Storage
  alias PoddyclipBackendWeb.LocaleHelpers

  # Main app - ensure guest user exists, serve SolidJS app
  def app(conn, _params) do
    conn = PoddyclipBackendWeb.Plugs.EnsureGuestUser.call(conn, [])

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:process)
  end

  def redirect_to_app(conn, _params) do
    locale = conn.assigns[:locale] || "en"
    redirect(conn, to: LocaleHelpers.locale_path(locale, "/app"))
  end

  def terms(conn, _params) do
    locale = conn.assigns[:locale] || "en"

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(locale_template(:terms, locale))
  end

  def privacy(conn, _params) do
    locale = conn.assigns[:locale] || "en"

    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(locale_template(:privacy, locale))
  end

  def help(conn, _params) do
    conn
    |> assign(:conn, conn)
    |> render(:help)
  end

  def legal(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:legal)
  end

  # Authenticated app
  def process(conn, _params) do
    # Use only root layout (no app layout) - root already has the navbar
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:process)
  end

  def past_munchings(conn, _params) do
    user = conn.assigns.current_scope.user
    jobs = load_job_history(user.id)

    conn
    |> put_layout(false)
    |> render(:past_munchings, jobs: jobs)
  end

  defp locale_template(page, "es"), do: :"#{page}_es"
  defp locale_template(page, "fr"), do: :"#{page}_fr"
  defp locale_template(page, "it"), do: :"#{page}_it"
  defp locale_template(page, _), do: page

  defp load_job_history(user_id) do
    jobs = Processing.list_completed_jobs_for_user(user_id)

    # Get all result keys and batch-check which exist
    keys = jobs |> Enum.map(& &1.result_s3_key) |> Enum.filter(& &1)
    existing_keys = Storage.filter_existing_keys(keys) |> MapSet.new()

    jobs
    |> Enum.filter(&(&1.result_s3_key && MapSet.member?(existing_keys, &1.result_s3_key)))
    |> Enum.flat_map(fn job ->
      filename = job.result_s3_key |> String.split("/") |> List.last() || "processed.mp3"

      case Storage.presign_download(job.result_s3_key, filename: filename) do
        {:ok, download_url} ->
          [%{
            id: job.id,
            filename: filename,
            inserted_at: job.inserted_at,
            download_url: download_url
          }]

        {:error, _reason} ->
          []
      end
    end)
  end
end
