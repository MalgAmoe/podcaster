defmodule PoddyclipBackend.Accounts.UserNotifier do
  import Swoosh.Email

  alias PoddyclipBackend.Mailer
  alias PoddyclipBackend.Accounts.User

  @app_name "Munchy Cow"
  @primary_color "#9333ea"
  @text_color "#1f2937"
  @muted_color "#6b7280"
  @bg_color "#f9fafb"

  defp from_email do
    case Application.get_env(:poddyclip_backend, :email_from) do
      nil -> {@app_name, "noreply@munchycow.com"}
      config -> {config[:name] || @app_name, config[:address] || "noreply@munchycow.com"}
    end
  end

  # Wraps content in a simple branded HTML template
  defp html_template(content, opts) do
    button_text = opts[:button_text]
    button_url = opts[:button_url]
    new_tab = opts[:new_tab] || false

    target_attr = if new_tab, do: ~s(target="_blank" rel="noopener noreferrer"), else: ""

    button_html = if button_text && button_url do
      """
      <div style="text-align: center; margin: 30px 0;">
        <a href="#{button_url}" #{target_attr} style="background-color: #{@primary_color}; color: white; padding: 12px 30px; text-decoration: none; border-radius: 6px; font-weight: 600; display: inline-block;">#{button_text}</a>
      </div>
      """
    else
      ""
    end

    """
    <!DOCTYPE html>
    <html>
    <head>
      <meta charset="utf-8">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
    </head>
    <body style="margin: 0; padding: 0; background-color: #{@bg_color}; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <div style="max-width: 500px; margin: 0 auto; padding: 40px 20px;">
        <div style="background-color: white; border-radius: 12px; padding: 40px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
          <div style="text-align: center; margin-bottom: 30px;">
            <span style="font-size: 24px; font-weight: 700; color: #{@text_color};">#{@app_name}</span>
          </div>
          <div style="color: #{@text_color}; font-size: 16px; line-height: 1.6;">
            #{content}
          </div>
          #{button_html}
        </div>
        <div style="text-align: center; margin-top: 20px; color: #{@muted_color}; font-size: 14px;">
          <a href="#{app_url()}" style="color: #{@muted_color}; text-decoration: none;">munchycow.com</a>
        </div>
      </div>
    </body>
    </html>
    """
  end

  # Delivers the email using the application mailer.
  defp deliver(recipient, subject, text_body, html_body) do
    email =
      new()
      |> to(recipient)
      |> from(from_email())
      |> subject(subject)
      |> text_body(text_body)

    email = if html_body, do: html_body(email, html_body), else: email

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
    text = """
    Hi #{user.email},

    You can change your email by visiting the URL below:

    #{url}

    If you didn't request this change, please ignore this.
    """

    html = html_template(
      "<p>Hi there,</p><p>You can change your email by clicking the button below.</p><p>If you didn't request this change, please ignore this.</p>",
      button_text: "Update Email",
      button_url: url
    )

    deliver(user.email, "Update email instructions", text, html)
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
    text = """
    Hi #{user.email},

    You can log into your account by visiting the URL below:

    #{url}

    If you didn't request this email, please ignore this.
    """

    html = html_template(
      "<p>Hi there,</p><p>Click the button below to log in to your account.</p><p style=\"color: #{@muted_color}; font-size: 14px;\">If you didn't request this email, please ignore it.</p>",
      button_text: "Log In",
      button_url: url
    )

    deliver(user.email, "Log in to Munchy Cow", text, html)
  end

  defp deliver_confirmation_instructions(user, url) do
    text = """
    Hi #{user.email},

    You can confirm your account by visiting the URL below:

    #{url}

    If you didn't create an account with us, please ignore this.
    """

    html = html_template(
      "<p>Hi there,</p><p>Welcome to Munchy Cow! Click the button below to confirm your account and get started.</p><p style=\"color: #{@muted_color}; font-size: 14px;\">If you didn't create an account, please ignore this.</p>",
      button_text: "Confirm Account",
      button_url: url
    )

    deliver(user.email, "Welcome to Munchy Cow", text, html)
  end

  @doc """
  Deliver notification that a job has completed.
  """
  def deliver_job_complete(user, job) do
    filename = job.filename || "your file"
    download_url = job.download_url || "#{app_url()}/app"

    text = """
    Hi there,

    Good news - #{@app_name} finished chewing on "#{filename}".

    Download your file:
    #{download_url}

    This link expires in 7 days.
    """

    html = html_template(
      "<p>Hi there,</p><p>Good news - I finished chewing on <strong>#{filename}</strong>.</p><p style=\"color: #{@muted_color}; font-size: 14px;\">This link expires in 7 days.</p>",
      button_text: "Download",
      button_url: download_url,
      new_tab: true
    )

    deliver(user.email, "Your audio is ready!", text, html)
  end

  @doc """
  Deliver notification that a job has failed.
  """
  def deliver_job_failed(user, job, error \\ nil) do
    filename = job.filename || "your file"
    error_message = error || job.error || "An unexpected error occurred"
    app_link = "#{app_url()}/app"

    text = """
    Hi there,

    #{@app_name} had trouble with "#{filename}".

    Error: #{error_message}

    Your minutes were refunded.

    Try again:
    #{app_link}
    """

    html = html_template(
      "<p>Hi there,</p><p>I had trouble with <strong>#{filename}</strong>.</p><p style=\"background-color: #fef2f2; padding: 12px; border-radius: 6px; color: #991b1b;\">#{error_message}</p><p>Don't worry - your time was refunded.</p>",
      button_text: "Try Again",
      button_url: app_link
    )

    deliver(user.email, "Oops - something went wrong", text, html)
  end

  @doc """
  Deliver notification that the user is running low on time.
  """
  def deliver_low_time(user, minutes_remaining, percent_used) do
    app_link = "#{app_url()}/account"

    text = """
    Hi there,

    You've used #{percent_used}% of your monthly processing time. You have #{minutes_remaining} minutes left.

    Upgrade to Munch Plan for 15 hours/month, or grab a Snack for extra minutes:
    #{app_link}
    """

    html = html_template(
      "<p>Hi there,</p><p>You've used <strong>#{percent_used}%</strong> of your processing time. You have <strong>#{minutes_remaining} minutes</strong> left.</p><p>Upgrade to Munch Plan for 15 hours/month, or grab a Snack for extra minutes.</p>",
      button_text: "View Options",
      button_url: app_link
    )

    deliver(user.email, "Running low on processing time", text, html)
  end

  @doc """
  Deliver notification that the user's subscription is expiring.
  """
  def deliver_subscription_expiring(user, days_remaining, end_date) do
    app_link = "#{app_url()}/account"

    text = """
    Hi there,

    Your cancelled subscription ends on #{end_date}. After that, you'll be on the Free plan (3 hours/month).

    Changed your mind? Reactivate here:
    #{app_link}
    """

    html = html_template(
      "<p>Hi there,</p><p>Your cancelled subscription ends on <strong>#{end_date}</strong>.</p><p>After that, you'll be on the Free plan (3 hours/month).</p><p>Changed your mind?</p>",
      button_text: "Reactivate",
      button_url: app_link
    )

    deliver(user.email, "Your Munch Plan ends in #{days_remaining} days", text, html)
  end
end
