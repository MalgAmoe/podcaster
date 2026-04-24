defmodule PoddyclipBackendWeb.HealthController do
  @moduledoc """
  Liveness/readiness probe for the web app.
  """
  use PoddyclipBackendWeb, :controller

  # alias PoddyclipBackend.Processing.Client

  def index(conn, _params) do
    # TODO: When revenue is high enough that keeping the Rust worker warm is acceptable,
    # restore the worker reachability check here. For now `/health` must stay local-only
    # so Fly health probes do not keep the processing service awake.
    #
    # worker_status = check_worker()
    #
    # json(conn, %{
    #   status: if(worker_status == "ok", do: "ok", else: "degraded"),
    #   worker_status: worker_status
    # })

    json(conn, %{
      status: "ok"
    })
  end

  # defp check_worker do
  #   case Client.health() do
  #     {:ok, _body} -> "ok"
  #     {:error, _reason} -> "unavailable"
  #   end
  # end
end
