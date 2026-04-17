defmodule PoddyclipBackendWeb.UserRegistrationControllerTest do
  use PoddyclipBackendWeb.ConnCase, async: true

  import PoddyclipBackend.AccountsFixtures

  describe "GET /users/register" do
    # Note: standalone registration is parked; the public auth flow redirects
    # old register URLs into unified sign-in.
    test "redirects to login page", %{conn: conn} do
      conn = get(conn, ~p"/users/register")
      assert redirected_to(conn) == ~p"/users/log-in"
    end

    test "redirects to app if already logged in", %{conn: conn} do
      conn = conn |> log_in_user(user_fixture()) |> get(~p"/users/register")
      assert redirected_to(conn) == ~p"/app"
    end
  end
end
