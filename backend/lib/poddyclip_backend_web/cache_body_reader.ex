defmodule PoddyclipBackendWeb.CacheBodyReader do
  @moduledoc """
  A custom body reader that caches the raw request body.

  This is needed for webhook signature verification where we need access
  to the exact raw body that was signed. Plug.Parsers normally consumes
  the body, making it unavailable for signature verification.

  ## Usage

  Configure in `endpoint.ex`:

      plug Plug.Parsers,
        parsers: [:urlencoded, :multipart, :json],
        body_reader: {PoddyclipBackendWeb.CacheBodyReader, :read_body, []},
        ...

  Then access the raw body in your controller:

      raw_body = conn.assigns[:raw_body]
  """

  @doc """
  Reads the request body and caches it in conn.assigns[:raw_body].

  This function is called by Plug.Parsers and must conform to the
  body reader spec: returns `{:ok, binary, conn}` or `{:more, binary, conn}`.
  """
  def read_body(conn, opts) do
    case Plug.Conn.read_body(conn, opts) do
      {:ok, body, conn} ->
        # Cache the complete body
        conn = cache_body(conn, body)
        {:ok, body, conn}

      {:more, body, conn} ->
        # For large bodies, we need to accumulate chunks
        conn = cache_body(conn, body)
        {:more, body, conn}

      {:error, reason} ->
        {:error, reason}
    end
  end

  defp cache_body(conn, body) do
    existing = conn.assigns[:raw_body] || ""
    Plug.Conn.assign(conn, :raw_body, existing <> body)
  end
end
