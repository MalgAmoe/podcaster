defmodule PoddyclipBackendWeb.UserSessionHTML do
  use PoddyclipBackendWeb, :html

  embed_templates "user_session_html/*"

  defp local_mail_adapter? do
    Application.get_env(:poddyclip_backend, PoddyclipBackend.Mailer)[:adapter] == Swoosh.Adapters.Local
  end
end
