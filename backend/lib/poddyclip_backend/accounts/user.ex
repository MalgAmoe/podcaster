defmodule PoddyclipBackend.Accounts.User do
  use Ecto.Schema
  import Ecto.Changeset

  @notification_types ["job_complete", "job_failed", "low_time", "subscription_expiry"]
  def notification_types, do: @notification_types

  schema "users" do
    field :email, :string
    field :password, :string, virtual: true, redact: true
    field :hashed_password, :string, redact: true
    field :confirmed_at, :utc_datetime
    field :authenticated_at, :utc_datetime, virtual: true

    # Billing fields
    belongs_to :plan, PoddyclipBackend.Billing.Plan
    field :seconds_available, :integer, default: 10_800
    field :seconds_allocated, :integer, default: 10_800
    field :subscription_status, :string, default: "none"
    field :polar_customer_id, :string
    field :polar_subscription_id, :string
    field :current_period_ends_at, :utc_datetime

    # Notification preferences
    field :notification_preferences, :map,
      default: Map.new(@notification_types, &{&1, true})
    field :last_low_time_notification_at, :utc_datetime
    field :expiry_notification_sent_at, :utc_datetime

    timestamps(type: :utc_datetime)
  end

  @doc """
  A user changeset for registering or changing the email.

  It requires the email to change otherwise an error is added.

  ## Options

    * `:validate_unique` - Set to false if you don't want to validate the
      uniqueness of the email, useful when displaying live validations.
      Defaults to `true`.
  """
  def email_changeset(user, attrs, opts \\ []) do
    user
    |> cast(attrs, [:email])
    |> validate_email(opts)
  end

  defp validate_email(changeset, opts) do
    changeset =
      changeset
      |> validate_required([:email])
      |> validate_format(:email, ~r/^[^@,;\s]+@[^@,;\s]+$/,
        message: "must have the @ sign and no spaces"
      )
      |> validate_length(:email, max: 160)

    if Keyword.get(opts, :validate_unique, true) do
      changeset
      |> unsafe_validate_unique(:email, PoddyclipBackend.Repo)
      |> unique_constraint(:email)
      |> validate_email_changed()
    else
      changeset
    end
  end

  defp validate_email_changed(changeset) do
    if get_field(changeset, :email) && get_change(changeset, :email) == nil do
      add_error(changeset, :email, "did not change")
    else
      changeset
    end
  end

  @doc """
  A user changeset for changing the password.

  It is important to validate the length of the password, as long passwords may
  be very expensive to hash for certain algorithms.

  ## Options

    * `:hash_password` - Hashes the password so it can be stored securely
      in the database and ensures the password field is cleared to prevent
      leaks in the logs. If password hashing is not needed and clearing the
      password field is not desired (like when using this changeset for
      validations on a LiveView form), this option can be set to `false`.
      Defaults to `true`.
  """
  def password_changeset(user, attrs, opts \\ []) do
    user
    |> cast(attrs, [:password])
    |> validate_confirmation(:password, message: "does not match password")
    |> validate_password(opts)
  end

  defp validate_password(changeset, opts) do
    changeset
    |> validate_required([:password])
    |> validate_length(:password, min: 12, max: 72)
    # Examples of additional password validation:
    # |> validate_format(:password, ~r/[a-z]/, message: "at least one lower case character")
    # |> validate_format(:password, ~r/[A-Z]/, message: "at least one upper case character")
    # |> validate_format(:password, ~r/[!?@#$%^&*_0-9]/, message: "at least one digit or punctuation character")
    |> maybe_hash_password(opts)
  end

  defp maybe_hash_password(changeset, opts) do
    hash_password? = Keyword.get(opts, :hash_password, true)
    password = get_change(changeset, :password)

    if hash_password? && password && changeset.valid? do
      changeset
      # If using Bcrypt, then further validate it is at most 72 bytes long
      |> validate_length(:password, max: 72, count: :bytes)
      # Hashing could be done with `Ecto.Changeset.prepare_changes/2`, but that
      # would keep the database transaction open longer and hurt performance.
      |> put_change(:hashed_password, Bcrypt.hash_pwd_salt(password))
      |> delete_change(:password)
    else
      changeset
    end
  end

  @doc """
  Confirms the account by setting `confirmed_at`.
  """
  def confirm_changeset(user) do
    now = DateTime.utc_now(:second)
    change(user, confirmed_at: now)
  end

  @doc """
  Verifies the password.

  If there is no user or the user doesn't have a password, we call
  `Bcrypt.no_user_verify/0` to avoid timing attacks.
  """
  def valid_password?(%PoddyclipBackend.Accounts.User{hashed_password: hashed_password}, password)
      when is_binary(hashed_password) and byte_size(password) > 0 do
    Bcrypt.verify_pass(password, hashed_password)
  end

  def valid_password?(_, _) do
    Bcrypt.no_user_verify()
    false
  end

  @doc """
  A changeset for updating notification preferences.
  """
  def notification_preferences_changeset(user, attrs) do
    user
    |> cast(attrs, [:notification_preferences])
    |> validate_notification_preferences()
  end

  defp validate_notification_preferences(changeset) do
    case get_change(changeset, :notification_preferences) do
      nil ->
        changeset

      prefs when is_map(prefs) ->
        valid_keys = MapSet.new(@notification_types)
        pref_keys = MapSet.new(Map.keys(prefs))

        if MapSet.subset?(pref_keys, valid_keys) do
          # Ensure all values are booleans
          if Enum.all?(Map.values(prefs), &is_boolean/1) do
            changeset
          else
            add_error(changeset, :notification_preferences, "all values must be booleans")
          end
        else
          add_error(changeset, :notification_preferences, "contains invalid keys")
        end

      _ ->
        add_error(changeset, :notification_preferences, "must be a map")
    end
  end

  @doc """
  Returns whether the user has a specific notification enabled.
  """
  def notification_enabled?(user, type) when is_atom(type) do
    notification_enabled?(user, Atom.to_string(type))
  end

  def notification_enabled?(user, type) when is_binary(type) do
    prefs = user.notification_preferences || %{}
    Map.get(prefs, type, true)
  end
end
