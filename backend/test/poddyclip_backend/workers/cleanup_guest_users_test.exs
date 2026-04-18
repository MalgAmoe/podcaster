defmodule PoddyclipBackend.Workers.CleanupGuestUsersTest do
  use PoddyclipBackend.DataCase

  import Ecto.Query

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackend.Accounts.User
  alias PoddyclipBackend.Feedback.Entry
  alias PoddyclipBackend.Processing.Job
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Workers.CleanupGuestUsers

  @retention_days 30

  test "deletes stale guest users with no recent session, jobs, or feedback" do
    guest = stale_guest_fixture()

    assert Repo.get(User, guest.id)

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 100) == 1

    refute Repo.get(User, guest.id)
  end

  test "does not delete recent guest users" do
    guest = guest_fixture()

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 100) == 0
    assert Repo.get(User, guest.id)
  end

  test "does not delete guest users with a recent session token" do
    guest = stale_guest_fixture()
    insert_recent_session_token(guest)

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 100) == 0
    assert Repo.get(User, guest.id)
  end

  test "does not delete guest users with jobs" do
    guest = stale_guest_fixture()
    insert_job(guest)

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 100) == 0
    assert Repo.get(User, guest.id)
  end

  test "does not delete guest users with feedback" do
    guest = stale_guest_fixture()
    insert_feedback(guest)

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 100) == 0
    assert Repo.get(User, guest.id)
  end

  test "deletes stale guest users in batches" do
    guests = Enum.map(1..3, fn _ -> stale_guest_fixture() end)

    assert CleanupGuestUsers.cleanup_stale_guest_users(@retention_days, 2) == 3

    refute Repo.exists?(from(u in User, where: u.id in ^Enum.map(guests, & &1.id)))
  end

  defp guest_fixture do
    {:ok, guest} = Accounts.create_guest_user()
    guest
  end

  defp stale_guest_fixture do
    guest = guest_fixture()
    stale_inserted_at = DateTime.utc_now() |> DateTime.add(-31, :day) |> DateTime.truncate(:second)

    Repo.update_all(from(u in User, where: u.id == ^guest.id), set: [inserted_at: stale_inserted_at])

    Repo.get!(User, guest.id)
  end

  defp insert_recent_session_token(guest) do
    {_, user_token} = Accounts.UserToken.build_session_token(guest)
    Repo.insert!(user_token)
  end

  defp insert_job(guest) do
    %Job{}
    |> Job.changeset(%{
      filename: "guest.wav",
      status: :completed,
      user_id: guest.id
    })
    |> Repo.insert!()
  end

  defp insert_feedback(guest) do
    %Entry{}
    |> Entry.changeset(%{
      user_id: guest.id,
      rating: "ok"
    })
    |> Repo.insert!()
  end
end
