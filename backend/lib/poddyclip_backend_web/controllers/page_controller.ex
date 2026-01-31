defmodule PoddyclipBackendWeb.PageController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Processing
  alias PoddyclipBackend.Storage
  alias PoddyclipBackendWeb.LocaleHelpers

  # Public pages
  def landing(conn, _params) do
    # Redirect logged-in users to the app
    if conn.assigns[:current_scope] do
      locale = conn.assigns[:locale] || "en"
      redirect(conn, to: LocaleHelpers.locale_path(locale, "/app"))
    else
      conn
      |> put_layout(false)
      |> assign(:conn, conn)
      |> render(:landing)
    end
  end

  def pricing(conn, _params) do
    # Redirect logged-in users to account page (where they can upgrade)
    if conn.assigns[:current_scope] do
      locale = conn.assigns[:locale] || "en"
      redirect(conn, to: LocaleHelpers.locale_path(locale, "/account"))
    else
      conn
      |> put_layout(false)
      |> assign(:conn, conn)
      |> render(:pricing)
    end
  end

  def terms(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:terms)
  end

  def privacy(conn, _params) do
    conn
    |> put_layout(false)
    |> assign(:conn, conn)
    |> render(:privacy)
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

  defp load_job_history(user_id) do
    jobs = Processing.list_completed_jobs_for_user(user_id)

    # Get all result keys and batch-check which exist
    keys = jobs |> Enum.map(& &1.result_s3_key) |> Enum.filter(& &1)
    existing_keys = Storage.filter_existing_keys(keys) |> MapSet.new()

    jobs
    |> Enum.filter(&(&1.result_s3_key && MapSet.member?(existing_keys, &1.result_s3_key)))
    |> Enum.map(fn job ->
      filename = job.result_s3_key |> String.split("/") |> List.last() || "processed.mp3"
      {:ok, download_url} = Storage.presign_download(job.result_s3_key, filename: filename)
      %{
        id: job.id,
        filename: filename,
        inserted_at: job.inserted_at,
        download_url: download_url
      }
    end)
  end
end
