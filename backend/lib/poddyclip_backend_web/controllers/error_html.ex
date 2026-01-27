defmodule PoddyclipBackendWeb.ErrorHTML do
  @moduledoc """
  This module is invoked by your endpoint in case of errors on HTML requests.

  See config/config.exs.
  """
  use PoddyclipBackendWeb, :html

  embed_templates "error_html/*"

  # Fallback for any status codes not covered by templates
  def render(template, _assigns) do
    Phoenix.Controller.status_message_from_template(template)
  end
end
