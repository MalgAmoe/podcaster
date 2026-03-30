defmodule PoddyclipBackend.Feedback do
  @moduledoc """
  Context for collecting user feedback on processing results.
  """

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Feedback.Entry

  def create_feedback(attrs) do
    %Entry{}
    |> Entry.changeset(attrs)
    |> Repo.insert()
  end
end
