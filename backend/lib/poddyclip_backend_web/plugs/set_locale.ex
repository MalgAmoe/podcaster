defmodule PoddyclipBackendWeb.Plugs.SetLocale do
  @moduledoc """
  Plug for setting the locale based on the connection assigns.

  This plug reads the :locale assign (set by the router pipelines)
  and configures Gettext accordingly.
  """

  import Plug.Conn

  @locales ~w(en es)
  @default_locale "en"

  def init(opts), do: opts

  def call(conn, _opts) do
    locale = conn.assigns[:locale] || @default_locale

    # Validate locale
    locale =
      if locale in @locales do
        locale
      else
        @default_locale
      end

    Gettext.put_locale(PoddyclipBackendWeb.Gettext, locale)

    conn
    |> assign(:locale, locale)
    |> put_session(:locale, locale)
  end

  @doc """
  Returns the list of supported locales.
  """
  def supported_locales, do: @locales

  @doc """
  Returns the default locale.
  """
  def default_locale, do: @default_locale
end
