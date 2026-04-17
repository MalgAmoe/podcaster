defmodule PoddyclipBackendWeb.HealthController do
  @moduledoc """
  Liveness/readiness probe for k8s. Reports Rust API reachability.
  """
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Processing.Client

  def index(conn, _params) do
    worker_status = check_worker()

    json(conn, %{
      status: if(worker_status == "ok", do: "ok", else: "degraded"),
      worker_status: worker_status
    })
  end

  defp check_worker do
    case Client.health() do
      {:ok, _body} -> "ok"
      {:error, _reason} -> "unavailable"
    end
  end
end
