defmodule PoddyclipBackendWeb.PageControllerTest do
  use PoddyclipBackendWeb.ConnCase, async: true

  import PoddyclipBackend.AccountsFixtures

  describe "GET /" do
    test "renders the landing page", %{conn: conn} do
      html =
        conn
        |> get(~p"/")
        |> html_response(200)

      assert html =~ "Make your voice recordings sound clean"
      assert html =~ "solid-landing-trial"
      refute html =~ "solid-process-app"
    end

    test "redirects signed-in users to the app", %{conn: conn} do
      user = user_fixture()

      conn =
        conn
        |> log_in_user(user)
        |> get(~p"/")

      assert redirected_to(conn) == ~p"/app"
    end
  end

  describe "GET /app" do
    test "redirects guests to the landing page", %{conn: conn} do
      conn =
        conn
        |> get(~p"/app")

      assert redirected_to(conn) == ~p"/"
    end

    test "renders the app shell for signed-in users", %{conn: conn} do
      user = user_fixture()

      html =
        conn
        |> log_in_user(user)
        |> get(~p"/app")
        |> html_response(200)

      assert html =~ "solid-process-app"
    end
  end
end
