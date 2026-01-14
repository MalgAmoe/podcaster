defmodule PoddyclipBackend.Repo do
  use Ecto.Repo,
    otp_app: :poddyclip_backend,
    adapter: Ecto.Adapters.Postgres
end
