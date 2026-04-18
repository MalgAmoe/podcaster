defmodule PoddyclipBackend.Workers.CleanupGuestUsers do
  @moduledoc """
  Oban worker that removes stale, empty guest users.

  Runs daily to delete guest users that:
  - are older than the configured retention window
  - have no recent session token
  - have no jobs
  - have no feedback

  This keeps guest-user growth under control without deleting active or
  meaningful guest records.
  """

  use Oban.Worker,
    queue: :default,
    max_attempts: 3

  import Ecto.Query
  require Logger

  alias PoddyclipBackend.Accounts.{User, UserToken}
  alias PoddyclipBackend.Feedback.Entry
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Repo

  @default_retention_days 30
  @default_batch_size 100
  @session_validity_days 14

  @impl Oban.Worker
  def perform(_job) do
    config = Application.get_env(:poddyclip_backend, :cleanup, [])
    retention_days = Keyword.get(config, :guest_retention_days, @default_retention_days)
    batch_size = Keyword.get(config, :guest_cleanup_batch_size, @default_batch_size)

    Logger.info(
      "Starting guest user cleanup (retention: #{retention_days} days, batch size: #{batch_size})"
    )

    deleted_count = cleanup_stale_guest_users(retention_days, batch_size)

    Logger.info("Cleanup complete: #{deleted_count} stale guest users deleted")

    :ok
  end

  def cleanup_stale_guest_users(
        retention_days \\ @default_retention_days,
        batch_size \\ @default_batch_size
      ) do
    guest_cutoff = DateTime.utc_now() |> DateTime.add(-retention_days, :day)
    session_cutoff = DateTime.utc_now() |> DateTime.add(-@session_validity_days, :day)

    do_cleanup(guest_cutoff, session_cutoff, batch_size, 0)
  end

  defp do_cleanup(guest_cutoff, session_cutoff, batch_size, total_deleted) do
    guest_ids = stale_guest_ids(guest_cutoff, session_cutoff, batch_size)

    case guest_ids do
      [] ->
        total_deleted

      ids ->
        {deleted_count, _} =
          from(u in User, where: u.id in ^ids)
          |> Repo.delete_all()

        Logger.info("Deleted stale guest users", user_ids: ids)

        do_cleanup(guest_cutoff, session_cutoff, batch_size, total_deleted + deleted_count)
    end
  end

  defp stale_guest_ids(guest_cutoff, session_cutoff, batch_size) do
    recent_session_user_ids =
      from(t in UserToken,
        where: t.context == "session",
        where: t.inserted_at > ^session_cutoff,
        select: t.user_id
      )

    guest_user_ids_with_jobs =
      from(j in Job,
        select: j.user_id
      )

    guest_user_ids_with_feedback =
      from(f in Entry,
        select: f.user_id
      )

    from(u in User,
      where: u.is_guest == true,
      where: u.inserted_at < ^guest_cutoff,
      where: u.id not in subquery(recent_session_user_ids),
      where: u.id not in subquery(guest_user_ids_with_jobs),
      where: u.id not in subquery(guest_user_ids_with_feedback),
      order_by: [asc: u.inserted_at],
      limit: ^batch_size,
      select: u.id
    )
    |> Repo.all()
  end
end
