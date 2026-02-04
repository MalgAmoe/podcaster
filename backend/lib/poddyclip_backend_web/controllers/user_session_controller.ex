defmodule PoddyclipBackendWeb.UserSessionController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Accounts
  alias PoddyclipBackendWeb.UserAuth
  alias PoddyclipBackendWeb.LocaleHelpers

  def new(conn, _params) do
    email = get_in(conn.assigns, [:current_scope, Access.key(:user), Access.key(:email)])
    form = Phoenix.Component.to_form(%{"email" => email}, as: "user")

    conn
    |> assign(:conn, conn)
    |> render(:new, form: form)
  end

  # Redirect old register URLs to unified sign-in
  def redirect_to_login(conn, _params) do
    locale = conn.assigns[:locale] || "en"
    redirect(conn, to: LocaleHelpers.locale_path(locale, "/users/log-in"))
  end

  # magic link login
  def create(conn, %{"user" => %{"token" => token} = user_params} = params) do
    info =
      case params do
        %{"_action" => "confirmed"} -> gettext("User confirmed successfully.")
        _ -> gettext("Welcome back!")
      end

    case Accounts.login_user_by_magic_link(token) do
      {:ok, {user, _expired_tokens}} ->
        conn
        |> put_flash(:info, info)
        |> UserAuth.log_in_user(user, user_params)

      {:error, :not_found} ->
        conn
        |> put_flash(:error, gettext("The link is invalid or it has expired."))
        |> assign(:conn, conn)
        |> render(:new, form: Phoenix.Component.to_form(%{}, as: "user"))
    end
  end

  # magic link request - auto-creates user if not found
  def create(conn, %{"user" => %{"email" => email}}) do
    user = Accounts.get_user_by_email(email) || create_user_for_email(email)
    locale = conn.assigns[:locale] || "en"

    if user do
      # Build magic link URL with locale prefix
      Accounts.deliver_login_instructions(
        user,
        fn token ->
          base_url = PoddyclipBackendWeb.Endpoint.url()
          path = LocaleHelpers.locale_path(locale, "/users/log-in/#{token}")
          base_url <> path
        end
      )
    end

    # Same message whether new or existing (prevents email enumeration)
    conn
    |> put_flash(:info, gettext("Check your email for a sign-in link. (Check spam if you don't see it)"))
    |> redirect(to: LocaleHelpers.locale_path(locale, "/users/log-in"))
  end

  defp create_user_for_email(email) do
    case Accounts.register_user(%{email: email}) do
      {:ok, user} -> user
      {:error, _changeset} -> nil
    end
  end

  def confirm(conn, %{"token" => token}) do
    locale = conn.assigns[:locale] || "en"

    if user = Accounts.get_user_by_magic_link_token(token) do
      form = Phoenix.Component.to_form(%{"token" => token}, as: "user")

      conn
      |> assign(:user, user)
      |> assign(:form, form)
      |> assign(:conn, conn)
      |> render(:confirm)
    else
      conn
      |> put_flash(:error, gettext("Magic link is invalid or it has expired."))
      |> redirect(to: LocaleHelpers.locale_path(locale, "/users/log-in"))
    end
  end

  def delete(conn, _params) do
    conn
    |> put_flash(:info, gettext("Logged out successfully."))
    |> UserAuth.log_out_user()
  end
end
