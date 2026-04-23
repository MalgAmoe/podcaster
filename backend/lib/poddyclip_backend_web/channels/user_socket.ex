defmodule PoddyclipBackendWeb.UserSocket do
  use Phoenix.Socket

  alias PoddyclipBackend.Accounts

  channel "job:*", PoddyclipBackendWeb.JobChannel
  channel "preview:*", PoddyclipBackendWeb.PreviewChannel
  channel "user:*", PoddyclipBackendWeb.UserChannel

  @doc """
  Connect to socket using a signed user token.
  Token is generated in root layout with Phoenix.Token.sign.
  """
  @impl true
  def connect(%{"token" => token}, socket, _connect_info) do
    # Token expires after 24 hours (matches session)
    case Phoenix.Token.verify(socket, "user socket", token, max_age: 86_400) do
      {:ok, user_id} ->
        # Verify user still exists
        try do
          user = Accounts.get_user!(user_id)
          {:ok, assign(socket, :current_user_id, user.id)}
        rescue
          Ecto.NoResultsError -> :error
        end

      {:error, _reason} ->
        :error
    end
  end

  def connect(_params, _socket, _connect_info) do
    :error
  end

  @impl true
  def id(socket), do: "user_socket:#{socket.assigns.current_user_id}"
end
