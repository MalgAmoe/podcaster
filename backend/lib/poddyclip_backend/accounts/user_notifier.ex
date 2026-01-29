defmodule PoddyclipBackend.Accounts.UserNotifier do
  import Swoosh.Email

  alias PoddyclipBackend.Mailer
  alias PoddyclipBackend.Accounts.User

  @app_name "Munchy Cow"

  defp from_email do
    case Application.get_env(:poddyclip_backend, :email_from) do
      nil -> {@app_name, "noreply@munchycow.com"}
      config -> {config[:name] || @app_name, config[:address] || "noreply@munchycow.com"}
    end
  end

  # Delivers the email using the application mailer.
  defp deliver(recipient, subject, body) do
    email =
      new()
      |> to(recipient)
      |> from(from_email())
      |> subject(subject)
      |> text_body(body)

    with {:ok, _metadata} <- Mailer.deliver(email) do
      {:ok, email}
    end
  end

  defp app_url do
    Application.get_env(:poddyclip_backend, PoddyclipBackendWeb.Endpoint)[:url][:host]
    |> case do
      "localhost" -> "http://localhost:4000"
      host -> "https://#{host}"
    end
  end

  @doc """
  Deliver instructions to update a user email.
  """
  def deliver_update_email_instructions(user, url) do
    deliver(user.email, "Update email instructions", """

    ==============================

    Hi #{user.email},

    You can change your email by visiting the URL below:

    #{url}

    If you didn't request this change, please ignore this.

    ==============================
    """)
  end

  @doc """
  Deliver instructions to log in with a magic link.
  """
  def deliver_login_instructions(user, url) do
    case user do
      %User{confirmed_at: nil} -> deliver_confirmation_instructions(user, url)
      _ -> deliver_magic_link_instructions(user, url)
    end
  end

  defp deliver_magic_link_instructions(user, url) do
    deliver(user.email, "Log in instructions", """

    ==============================

    Hi #{user.email},

    You can log into your account by visiting the URL below:

    #{url}

    If you didn't request this email, please ignore this.

    ==============================
    """)
  end

  defp deliver_confirmation_instructions(user, url) do
    deliver(user.email, "Confirmation instructions", """

    ==============================

    Hi #{user.email},

    You can confirm your account by visiting the URL below:

    #{url}

    If you didn't create an account with us, please ignore this.

    ==============================
    """)
  end

  @doc """
  Deliver notification that a job has completed.
  """
  def deliver_job_complete(user, job) do
    filename = job.filename || "your file"
    download_url = job.download_url || "#{app_url()}/app"

    deliver(user.email, "Your audio is ready!", """

    ==============================

    Hi there,

    Good news - #{@app_name} finished chewing on "#{filename}".

    Download your file:
    #{download_url}

    This link expires in 7 days.

    ==============================
    """)
  end

  @doc """
  Deliver notification that a job has failed.
  """
  def deliver_job_failed(user, job, error \\ nil) do
    filename = job.filename || "your file"
    error_message = error || job.error || "An unexpected error occurred"
    app_link = "#{app_url()}/app"

    deliver(user.email, "Oops - something went wrong", """

    ==============================

    Hi there,

    #{@app_name} had trouble with "#{filename}".

    Error: #{error_message}

    Your minutes were refunded.

    Try again:
    #{app_link}

    ==============================
    """)
  end

  @doc """
  Deliver notification that the user is running low on minutes.
  """
  def deliver_low_minutes(user, minutes_remaining, percent_used) do
    app_link = "#{app_url()}/account"

    deliver(user.email, "Running low on minutes", """

    ==============================

    Hi there,

    You've used #{percent_used}% of your monthly minutes. You have #{minutes_remaining} minutes left.

    Upgrade to Pro for 900 minutes/month:
    #{app_link}

    ==============================
    """)
  end

  @doc """
  Deliver notification that the user's subscription is expiring.
  """
  def deliver_subscription_expiring(user, days_remaining, end_date) do
    app_link = "#{app_url()}/account"

    deliver(user.email, "Your Pro access ends in #{days_remaining} days", """

    ==============================

    Hi there,

    Your cancelled subscription ends on #{end_date}. After that, you'll be on the Free plan (15 minutes/month).

    Changed your mind? Reactivate here:
    #{app_link}

    ==============================
    """)
  end
end
