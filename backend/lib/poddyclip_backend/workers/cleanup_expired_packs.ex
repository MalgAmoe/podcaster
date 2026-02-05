defmodule PoddyclipBackend.Workers.CleanupExpiredPacks do
  @moduledoc """
  Oban worker that cleans up expired minute packs.

  Runs daily at 4am to:
  1. Delete packs that expired more than 30 days ago (keeps audit trail)

  Configuration:
    Configured in Oban cron in config/config.exs
  """

  use Oban.Worker,
    queue: :default,
    max_attempts: 3

  import Ecto.Query
  require Logger

  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Billing.MinutePack

  @retention_days 30

  @impl Oban.Worker
  def perform(_job) do
    Logger.info("Starting expired minute packs cleanup (retention: #{@retention_days} days)")

    deleted_count = delete_old_expired_packs()

    Logger.info("Cleanup complete: #{deleted_count} expired minute packs deleted")

    :ok
  end

  defp delete_old_expired_packs do
    # Delete packs that expired more than 30 days ago
    cutoff = DateTime.utc_now() |> DateTime.add(-@retention_days, :day)

    # Find packs to delete
    packs_to_delete =
      from(p in MinutePack,
        where: p.expires_at < ^cutoff,
        select: %{id: p.id, user_id: p.user_id, expires_at: p.expires_at}
      )
      |> Repo.all()

    if length(packs_to_delete) > 0 do
      Logger.info("Found #{length(packs_to_delete)} expired packs to delete",
        pack_ids: Enum.map(packs_to_delete, & &1.id)
      )

      pack_ids = Enum.map(packs_to_delete, & &1.id)

      {deleted_count, _} =
        from(p in MinutePack, where: p.id in ^pack_ids)
        |> Repo.delete_all()

      deleted_count
    else
      0
    end
  end
end
