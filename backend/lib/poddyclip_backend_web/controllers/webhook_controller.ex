defmodule PoddyclipBackendWeb.WebhookController do
  use PoddyclipBackendWeb, :controller
  require Logger

  alias PoddyclipBackend.Processing

  @doc """
  Receives job status updates from the Rust API.

  Expected payload:
  {
    "status": "processing" | "completed" | "failed",
    "progress": {"stage": "...", "percent_complete": 50},
    "error": "optional error message",
    "download_url": "optional presigned URL"
  }
  """
  def job_status(conn, %{"job_id" => job_id} = params) do
    Logger.metadata(job_id: job_id)

    with :ok <- verify_webhook_secret(conn),
         {:ok, job} <- Processing.update_job_status(job_id, params) do
      # Set user_id in metadata for this request's logs
      Logger.metadata(user_id: job.user_id)

      Logger.debug("Job webhook received",
        job_id: job_id,
        status: params["status"],
        stage: get_in(params, ["progress", "stage"])
      )
      json(conn, %{ok: true, job_id: job.id})
    else
      :unauthorized ->
        Logger.warning("Webhook auth failed", job_id: job_id)
        conn
        |> put_status(:unauthorized)
        |> json(%{error: "Invalid webhook secret"})

      {:error, :not_found} ->
        Logger.warning("Webhook for unknown job", job_id: job_id)
        conn
        |> put_status(:not_found)
        |> json(%{error: "Job not found"})

      {:error, reason} ->
        Logger.error("Webhook processing failed", job_id: job_id, error: inspect(reason))
        conn
        |> put_status(:unprocessable_entity)
        |> json(%{error: inspect(reason)})
    end
  end

  defp verify_webhook_secret(conn) do
    expected = Application.get_env(:poddyclip_backend, :webhook_secret)
    provided = get_req_header(conn, "x-webhook-secret") |> List.first()

    cond do
      # Secret matches
      expected && provided && Plug.Crypto.secure_compare(expected, provided) -> :ok
      # No secret configured - REJECT (security: don't allow unauthenticated requests)
      is_nil(expected) or expected == "" ->
        Logger.error("WEBHOOK_SECRET not configured - rejecting unauthenticated request")
        :unauthorized
      # Secret configured but not provided or doesn't match
      true -> :unauthorized
    end
  end
end
