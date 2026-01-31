defmodule PoddyclipBackendWeb.LocaleHook do
  @moduledoc """
  LiveView hook for setting the locale from session.

  This hook reads the locale from the session (set by the router/plug)
  and configures Gettext for the LiveView process.
  """

  import Phoenix.Component
  import Phoenix.LiveView

  @default_locale "en"
  @supported_locales ~w(en es)

  def on_mount(:set_locale, _params, session, socket) do
    locale = get_locale_from_session(session)

    # Set Gettext locale for this LiveView process
    Gettext.put_locale(PoddyclipBackendWeb.Gettext, locale)

    socket =
      socket
      |> assign(:locale, locale)
      |> attach_hook(:set_locale_on_handle_params, :handle_params, &handle_params_hook/3)

    {:cont, socket}
  end

  # Re-set locale on navigation (handle_params is called on each navigation)
  defp handle_params_hook(_params, _uri, socket) do
    locale = socket.assigns[:locale] || @default_locale
    Gettext.put_locale(PoddyclipBackendWeb.Gettext, locale)
    {:cont, socket}
  end

  defp get_locale_from_session(session) do
    locale = session["locale"] || @default_locale

    if locale in @supported_locales do
      locale
    else
      @default_locale
    end
  end
end
