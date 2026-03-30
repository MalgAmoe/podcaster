defmodule PoddyclipBackend.Feedback.Entry do
  use Ecto.Schema
  import Ecto.Changeset

  schema "feedback" do
    field :rating, :string
    field :prompt_key, :string
    field :value, :string

    belongs_to :job, PoddyclipBackend.Processing.Job
    belongs_to :user, PoddyclipBackend.Accounts.User

    timestamps(type: :utc_datetime, updated_at: false)
  end

  def changeset(entry, attrs) do
    entry
    |> cast(attrs, [:job_id, :user_id, :rating, :prompt_key, :value])
    |> validate_required([:user_id])
    |> validate_inclusion(:rating, ["great", "ok", "not_good"])
  end
end
