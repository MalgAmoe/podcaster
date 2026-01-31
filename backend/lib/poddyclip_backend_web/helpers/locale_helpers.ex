defmodule PoddyclipBackendWeb.LocaleHelpers do
  @moduledoc """
  Helper functions for locale-aware paths and URLs.
  """

  @doc """
  Returns the path with locale prefix.
  For English (default), returns the path as-is.
  For Spanish, adds /es prefix.
  """
  def locale_path(nil, path), do: path
  def locale_path("en", path), do: path
  def locale_path("es", path), do: "/es" <> path
  def locale_path(_locale, path), do: path

  @doc """
  Returns the path to switch to a different locale.
  Handles the current request path and either adds or removes the /es prefix.
  """
  def switch_locale_path(assigns, new_locale) do
    # Get current path from conn or socket
    current_path = get_current_path(assigns)
    current_locale = assigns[:locale] || "en"

    cond do
      # Already on target locale
      new_locale == current_locale ->
        current_path

      # Switching to Spanish: add /es prefix
      new_locale == "es" ->
        # Remove any existing /es prefix first, then add it
        path_without_es = String.replace_prefix(current_path, "/es", "")
        "/es" <> path_without_es

      # Switching to English: remove /es prefix
      new_locale == "en" ->
        path = String.replace_prefix(current_path, "/es", "")
        if path == "", do: "/", else: path

      true ->
        current_path
    end
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
