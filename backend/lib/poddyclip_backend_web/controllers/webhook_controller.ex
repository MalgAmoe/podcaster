defmodule PoddyclipBackendWeb.WebhookController do
  use PoddyclipBackendWeb, :controller

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
    with :ok <- verify_webhook_secret(conn),
         {:ok, job} <- Processing.update_job_status(job_id, params) do
      json(conn, %{ok: true, job_id: job.id})
    else
      :unauthorized ->
        conn
        |> put_status(:unauthorized)
        |> json(%{error: "Invalid webhook secret"})

      {:error, :not_found} ->
        conn
        |> put_status(:not_found)
        |> json(%{error: "Job not found"})

      {:error, reason} ->
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
      # No secret configured - allow (dev mode, log warning)
      is_nil(expected) ->
        require Logger
        Logger.warning("Webhook secret not configured - allowing unauthenticated request")
        :ok
      # Secret configured but not provided or doesn't match
      true -> :unauthorized
    end
  end
end
