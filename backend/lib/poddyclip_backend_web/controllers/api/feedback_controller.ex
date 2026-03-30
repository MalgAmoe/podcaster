defmodule PoddyclipBackendWeb.Api.FeedbackController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Feedback

  def create(conn, params) do
    user = conn.assigns.current_user

    attrs = %{
      user_id: user.id,
      job_id: params["job_id"],
      rating: params["rating"],
      prompt_key: params["prompt_key"],
      value: params["value"]
    }

    case Feedback.create_feedback(attrs) do
      {:ok, _} -> json(conn, %{ok: true})
      {:error, _} -> conn |> put_status(400) |> json(%{error: "Invalid feedback"})
    end
  end
end
