defmodule PoddyclipBackend.Accounts do
  @moduledoc """
  The Accounts context.
  """

  import Ecto.Query, warn: false
  alias PoddyclipBackend.Repo

  alias PoddyclipBackend.Accounts.{User, UserToken, UserNotifier}

  ## Database getters

  @doc """
  Gets a user by email.

  ## Examples

      iex> get_user_by_email("foo@example.com")
      %User{}

      iex> get_user_by_email("unknown@example.com")
      nil

  """
  def get_user_by_email(email) when is_binary(email) do
    Repo.get_by(User, email: email)
  end

  @doc """
  Gets a user by email and password.

  ## Examples

      iex> get_user_by_email_and_password("foo@example.com", "correct_password")
      %User{}

      iex> get_user_by_email_and_password("foo@example.com", "invalid_password")
      nil

  """
  def get_user_by_email_and_password(email, password)
      when is_binary(email) and is_binary(password) do
    user = Repo.get_by(User, email: email)
    if User.valid_password?(user, password), do: user
  end

  @doc """
  Gets a single user.

  Raises `Ecto.NoResultsError` if the User does not exist.

  ## Examples

      iex> get_user!(123)
      %User{}

      iex> get_user!(456)
      ** (Ecto.NoResultsError)

  """
  def get_user!(id), do: Repo.get!(User, id)

  @doc """
  Gets a single user, returns nil if not found.
  """
  def get_user(id), do: Repo.get(User, id)

  ## User registration

  @doc """
  Registers a user.

  New users are assigned the free plan with 900 seconds (15 minutes).

  ## Examples

      iex> register_user(%{field: value})
      {:ok, %User{}}

      iex> register_user(%{field: bad_value})
      {:error, %Ecto.Changeset{}}

  """
  def register_user(attrs) do
    alias PoddyclipBackend.Billing

    free_plan = Billing.get_or_create_free_plan()
    promo = Billing.find_active_promo()

    Repo.transaction(fn ->
      # Claim promo slot first (atomic) — if exhausted, just skip the bonus
      promo_claimed =
        if promo do
          case Billing.claim_promo(promo) do
            {:ok, _} -> true
            {:error, :exhausted} -> false
          end
        else
          false
        end

      seconds = free_plan.seconds + if(promo_claimed, do: promo.bonus_seconds, else: 0)

      %User{}
      |> User.email_changeset(attrs)
      |> Ecto.Changeset.put_change(:plan_id, free_plan.id)
      |> Ecto.Changeset.put_change(:seconds_available, seconds)
      |> Ecto.Changeset.put_change(:seconds_allocated, seconds)
      |> Ecto.Changeset.put_change(
        :current_period_ends_at,
        DateTime.utc_now() |> DateTime.add(30, :day) |> DateTime.truncate(:second)
      )
      |> Repo.insert()
      |> case do
        {:ok, user} -> user
        {:error, changeset} -> Repo.rollback(changeset)
      end
    end)
  end

  ## Settings

  @doc """
  Checks whether the user is in sudo mode.

  The user is in sudo mode when the last authentication was done no further
  than 20 minutes ago. The limit can be given as second argument in minutes.
  """
  def sudo_mode?(user, minutes \\ -20)

  def sudo_mode?(%User{authenticated_at: ts}, minutes) when is_struct(ts, DateTime) do
    DateTime.after?(ts, DateTime.utc_now() |> DateTime.add(minutes, :minute))
  end

  def sudo_mode?(_user, _minutes), do: false

  @doc """
  Returns an `%Ecto.Changeset{}` for changing the user email.

  See `PoddyclipBackend.Accounts.User.email_changeset/3` for a list of supported options.

  ## Examples

      iex> change_user_email(user)
      %Ecto.Changeset{data: %User{}}

  """
  def change_user_email(user, attrs \\ %{}, opts \\ []) do
    User.email_changeset(user, attrs, opts)
  end

  @doc """
  Updates the user email using the given token.

  If the token matches, the user email is updated and the token is deleted.
  """
  def update_user_email(user, token) do
    context = "change:#{user.email}"

    Repo.transact(fn ->
      with {:ok, query} <- UserToken.verify_change_email_token_query(token, context),
           %UserToken{sent_to: email} <- Repo.one(query),
           {:ok, user} <- Repo.update(User.email_changeset(user, %{email: email})),
           {_count, _result} <-
             Repo.delete_all(from(UserToken, where: [user_id: ^user.id, context: ^context])) do
        {:ok, user}
      else
        _ -> {:error, :transaction_aborted}
      end
    end)
  end

  @doc """
  Returns an `%Ecto.Changeset{}` for changing the user password.

  See `PoddyclipBackend.Accounts.User.password_changeset/3` for a list of supported options.

  ## Examples

      iex> change_user_password(user)
      %Ecto.Changeset{data: %User{}}

  """
  def change_user_password(user, attrs \\ %{}, opts \\ []) do
    User.password_changeset(user, attrs, opts)
  end

  @doc """
  Updates the user password.

  Returns a tuple with the updated user, as well as a list of expired tokens.

  ## Examples

      iex> update_user_password(user, %{password: ...})
      {:ok, {%User{}, [...]}}

      iex> update_user_password(user, %{password: "too short"})
      {:error, %Ecto.Changeset{}}

  """
  def update_user_password(user, attrs) do
    user
    |> User.password_changeset(attrs)
    |> update_user_and_delete_all_tokens()
  end

  ## Session

  @doc """
  Generates a session token.
  """
  def generate_user_session_token(user) do
    {token, user_token} = UserToken.build_session_token(user)
    Repo.insert!(user_token)
    token
  end

  @doc """
  Gets the user with the given signed token.

  If the token is valid `{user, token_inserted_at}` is returned, otherwise `nil` is returned.
  """
  def get_user_by_session_token(token) do
    {:ok, query} = UserToken.verify_session_token_query(token)
    Repo.one(query)
  end

  @doc """
  Gets the user with the given magic link token.
  """
  def get_user_by_magic_link_token(token) do
    with {:ok, query} <- UserToken.verify_magic_link_token_query(token),
         {user, _token} <- Repo.one(query) do
      user
    else
      _ -> nil
    end
  end

  @doc """
  Logs the user in by magic link.

  There are three cases to consider:

  1. The user has already confirmed their email. They are logged in
     and the magic link is expired.

  2. The user has not confirmed their email and no password is set.
     In this case, the user gets confirmed, logged in, and all tokens -
     including session ones - are expired. In theory, no other tokens
     exist but we delete all of them for best security practices.

  3. The user has not confirmed their email but a password is set.
     This cannot happen in the default implementation but may be the
     source of security pitfalls. See the "Mixing magic link and password registration" section of
     `mix help phx.gen.auth`.
  """
  def login_user_by_magic_link(token) do
    {:ok, query} = UserToken.verify_magic_link_token_query(token)

    case Repo.one(query) do
      # Prevent session fixation attacks by disallowing magic links for unconfirmed users with password
      {%User{confirmed_at: nil, hashed_password: hash}, _token} when not is_nil(hash) ->
        raise """
        magic link log in is not allowed for unconfirmed users with a password set!

        This cannot happen with the default implementation, which indicates that you
        might have adapted the code to a different use case. Please make sure to read the
        "Mixing magic link and password registration" section of `mix help phx.gen.auth`.
        """

      {%User{confirmed_at: nil} = user, _token} ->
        user
        |> User.confirm_changeset()
        |> update_user_and_delete_all_tokens()

      {user, token} ->
        Repo.delete!(token)
        {:ok, {user, []}}

      nil ->
        {:error, :not_found}
    end
  end

  @doc ~S"""
  Delivers the update email instructions to the given user.

  ## Examples

      iex> deliver_user_update_email_instructions(user, current_email, &url(~p"/users/settings/confirm-email/#{&1}"))
      {:ok, %{to: ..., body: ...}}

  """
  def deliver_user_update_email_instructions(%User{} = user, current_email, update_email_url_fun)
      when is_function(update_email_url_fun, 1) do
    {encoded_token, user_token} = UserToken.build_email_token(user, "change:#{current_email}")

    Repo.insert!(user_token)
    UserNotifier.deliver_update_email_instructions(user, update_email_url_fun.(encoded_token))
  end

  @doc """
  Delivers the magic link login instructions to the given user.
  """
  def deliver_login_instructions(%User{} = user, magic_link_url_fun)
      when is_function(magic_link_url_fun, 1) do
    {encoded_token, user_token} = UserToken.build_email_token(user, "login")
    Repo.insert!(user_token)
    UserNotifier.deliver_login_instructions(user, magic_link_url_fun.(encoded_token))
  end

  @doc """
  Deletes the signed token with the given context.
  """
  def delete_user_session_token(token) do
    Repo.delete_all(from(UserToken, where: [token: ^token, context: "session"]))
    :ok
  end

  ## Token helper

  defp update_user_and_delete_all_tokens(changeset) do
    Repo.transact(fn ->
      with {:ok, user} <- Repo.update(changeset) do
        tokens_to_expire = Repo.all_by(UserToken, user_id: user.id)

        Repo.delete_all(from(t in UserToken, where: t.id in ^Enum.map(tokens_to_expire, & &1.id)))

        {:ok, {user, tokens_to_expire}}
      end
    end)
  end

  ## Notification Preferences

  @doc """
  Returns an `%Ecto.Changeset{}` for changing notification preferences.
  """
  def change_notification_preferences(user, attrs \\ %{}) do
    User.notification_preferences_changeset(user, attrs)
  end

  @doc """
  Updates the user's notification preferences.

  ## Examples

      iex> update_notification_preferences(user, %{"job_complete" => false})
      {:ok, %User{}}

  """
  def update_notification_preferences(user, new_prefs) when is_map(new_prefs) do
    # Merge with existing preferences
    current_prefs = user.notification_preferences || %{}
    merged_prefs = Map.merge(current_prefs, new_prefs)

    user
    |> User.notification_preferences_changeset(%{notification_preferences: merged_prefs})
    |> Repo.update()
  end

  @doc """
  Records that a low time notification was sent.
  Prevents spam by tracking last notification time.
  """
  def record_low_time_notification(user) do
    user
    |> Ecto.Changeset.change(%{last_low_time_notification_at: DateTime.utc_now(:second)})
    |> Repo.update()
  end

  @doc """
  Checks if we should send a low time notification.
  Returns false if one was sent in the current billing period.
  """
  def should_send_low_time_notification?(user) do
    case user.last_low_time_notification_at do
      nil ->
        true

      last_sent ->
        # Only send once per billing period
        # If subscription has a period end, use that; otherwise, use 30 days
        period_start =
          case user.current_period_ends_at do
            nil -> DateTime.add(DateTime.utc_now(), -30, :day)
            period_end -> DateTime.add(period_end, -30, :day)
          end

        DateTime.before?(last_sent, period_start)
    end
  end

  @doc """
  Records that an expiry notification was sent.
  """
  def record_expiry_notification(user) do
    user
    |> Ecto.Changeset.change(%{expiry_notification_sent_at: DateTime.utc_now(:second)})
    |> Repo.update()
  end

  @doc """
  Clears the expiry notification flag (e.g., when subscription is reactivated).
  """
  def clear_expiry_notification(user) do
    user
    |> Ecto.Changeset.change(%{expiry_notification_sent_at: nil})
    |> Repo.update()
  end

  ## Account Deletion

  @doc """
  Permanently deletes a user account and all associated data.

  This function:
  1. Revokes any active Polar subscription (immediate termination)
  2. Deletes all S3 files (inputs and results) for the user
  3. Deletes all user tokens
  4. Deletes the user record (cascades to jobs and minute_packs)

  Returns `{:ok, user}` on success, `{:error, reason}` on failure.
  """
  def delete_user_account(%User{} = user) do
    alias PoddyclipBackend.{Storage, Polar}
    require Logger

    Logger.info("Deleting user account", user_id: user.id, email: user.email)

    # 1. Revoke Polar subscription if active (immediate termination)
    if user.polar_subscription_id && user.subscription_status in ["active", "past_due"] do
      case Polar.revoke_subscription(user.polar_subscription_id) do
        {:ok, _} ->
          Logger.info("Revoked Polar subscription for deleted user",
            user_id: user.id,
            subscription_id: user.polar_subscription_id
          )

        {:error, reason} ->
          # Log but don't block deletion
          Logger.warning("Failed to revoke Polar subscription during account deletion",
            user_id: user.id,
            subscription_id: user.polar_subscription_id,
            reason: inspect(reason)
          )
      end
    end

    # 2. Delete all S3 files for this user
    delete_user_s3_files(user.id)

    # 3. Delete all tokens first (not strictly necessary due to cascade, but explicit)
    Repo.delete_all(from(t in UserToken, where: t.user_id == ^user.id))

    # 4. Delete the user (cascades to jobs, minute_packs via DB constraints)
    case Repo.delete(user) do
      {:ok, deleted_user} ->
        Logger.info("User account deleted successfully", user_id: user.id)
        {:ok, deleted_user}

      {:error, changeset} ->
        Logger.error("Failed to delete user account",
          user_id: user.id,
          error: inspect(changeset.errors)
        )
        {:error, changeset}
    end
  end

  defp delete_user_s3_files(user_id) do
    alias PoddyclipBackend.Storage
    require Logger

    # Delete input files
    case Storage.delete_user_inputs(user_id) do
      :ok -> Logger.debug("Deleted input files for user", user_id: user_id)
      {:error, reason} -> Logger.warning("Failed to delete input files", user_id: user_id, error: inspect(reason))
    end

    # Delete result files - list all keys under results/{user_id}/
    if Storage.enabled?() do
      prefix = "results/#{user_id}/"
      keys = Storage.list_keys(prefix)

      Enum.each(keys, fn key ->
        case Storage.delete(key) do
          {:ok, _} -> :ok
          {:error, reason} -> Logger.warning("Failed to delete S3 file", key: key, error: inspect(reason))
        end
      end)

      Logger.debug("Deleted #{length(keys)} result files for user", user_id: user_id)
    end

    :ok
  end
end
