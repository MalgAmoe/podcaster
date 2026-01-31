defmodule PoddyclipBackendWeb.LocaleHelpers do
  @moduledoc """
  Helper functions for locale-aware paths and URLs.
  """

  @supported_locales ~w(en es it fr)

  @doc """
  Returns the path with locale prefix.
  For English (default), returns the path as-is.
  For other locales, adds /{locale} prefix.
  """
  def locale_path(nil, path), do: path
  def locale_path("en", path), do: path
  def locale_path(locale, path) when locale in @supported_locales, do: "/" <> locale <> path
  def locale_path(_locale, path), do: path

  @doc """
  Returns the path to switch to a different locale.
  Handles the current request path and either adds or removes the locale prefix.
  """
  def switch_locale_path(assigns, new_locale) do
    # Get current path from conn or socket
    current_path = get_current_path(assigns)

    # Remove any existing locale prefix
    base_path = strip_locale_prefix(current_path)

    # Add new locale prefix (unless English)
    if new_locale == "en" do
      if base_path == "", do: "/", else: base_path
    else
      "/" <> new_locale <> base_path
    end
  end

  # Strip any locale prefix from the path
  defp strip_locale_prefix(path) do
    Enum.reduce(@supported_locales -- ["en"], path, fn locale, acc ->
      String.replace_prefix(acc, "/" <> locale, "")
    end)
  end

  defp get_current_path(assigns) do
    cond do
      # From a regular controller (conn is available)
      conn = assigns[:conn] ->
        conn.request_path

      # From LiveView (socket is available)
      socket = assigns[:socket] ->
        # Try to get from socket assigns or default to root
        socket.assigns[:current_path] || "/"

      # From LiveView uri
      uri = assigns[:uri] ->
        URI.parse(uri).path || "/"

      # Fallback
      true ->
        "/"
    end
  end
end
