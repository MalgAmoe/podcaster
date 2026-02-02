defmodule PoddyclipBackendWeb.AdminApiControllerTest do
  use PoddyclipBackendWeb.ConnCase

  # Skip tests if admin routes are not enabled
  if Application.compile_env(:poddyclip_backend, :admin_enabled) do
    alias PoddyclipBackend.Processing.Job
    alias PoddyclipBackend.Repo

    import PoddyclipBackend.AccountsFixtures

    setup %{conn: conn} do
      # Set admin credentials for tests
      Application.put_env(:poddyclip_backend, :admin_username, "admin")
      Application.put_env(:poddyclip_backend, :admin_password, "testpass")

      # Add basic auth header
      auth = Base.encode64("admin:testpass")

      conn =
        conn
        |> put_req_header("authorization", "Basic #{auth}")
        |> put_req_header("accept", "application/json")

      {:ok, conn: conn}
    end

    describe "GET /admin/api/health" do
      test "returns health status", %{conn: conn} do
        conn = get(conn, ~p"/admin/api/health")

        assert %{
                 "status" => status,
                 "checks" => checks,
                 "timestamp" => _timestamp
               } = json_response(conn, 200)

        assert status in ["healthy", "degraded", "unhealthy"]
        assert Map.has_key?(checks, "database")
        assert Map.has_key?(checks, "oban")
      end

      test "rejects unauthorized requests", %{conn: conn} do
        conn =
          conn
          |> delete_req_header("authorization")
          |> get(~p"/admin/api/health")

        assert conn.status == 401
      end

      test "rejects wrong credentials", %{conn: conn} do
        auth = Base.encode64("wrong:credentials")

        conn =
          conn
          |> put_req_header("authorization", "Basic #{auth}")
          |> get(~p"/admin/api/health")

        assert conn.status == 401
      end
    end

    describe "GET /admin/api/jobs/stats" do
      test "returns job statistics", %{conn: conn} do
        conn = get(conn, ~p"/admin/api/jobs/stats")

        assert %{
                 "current" => %{"queued" => _, "processing" => _},
                 "last_24h" => %{"completed" => _, "failed" => _, "success_rate" => _},
                 "timestamp" => _
               } = json_response(conn, 200)
      end

      test "reflects actual job counts", %{conn: conn} do
        user = user_fixture()
        insert_job(user.id, :queued)
        insert_job(user.id, :processing)

        conn = get(conn, ~p"/admin/api/jobs/stats")
        response = json_response(conn, 200)

        assert response["current"]["queued"] == 1
        assert response["current"]["processing"] == 1
      end
    end

    describe "GET /admin/api/errors" do
      test "returns recent errors", %{conn: conn} do
        conn = get(conn, ~p"/admin/api/errors")

        assert %{
                 "errors" => errors,
                 "total_24h" => _
               } = json_response(conn, 200)

        assert is_list(errors)
      end

      test "respects limit parameter", %{conn: conn} do
        user = user_fixture()

        for i <- 1..5 do
          insert_job(user.id, :failed, error: "Error #{i}")
        end

        conn = get(conn, ~p"/admin/api/errors?limit=2")
        response = json_response(conn, 200)

        assert length(response["errors"]) == 2
        assert response["total_24h"] == 5
      end
    end

    describe "GET /admin/api/jobs/active" do
      test "returns active jobs", %{conn: conn} do
        conn = get(conn, ~p"/admin/api/jobs/active")

        assert %{
                 "jobs" => jobs,
                 "count" => count,
                 "timestamp" => _
               } = json_response(conn, 200)

        assert is_list(jobs)
        assert count == length(jobs)
      end
    end

    describe "GET /admin/api/users/stats" do
      test "returns user statistics", %{conn: conn} do
        _user = user_fixture()

        conn = get(conn, ~p"/admin/api/users/stats")

        assert %{
                 "users" => %{
                   "total" => total,
                   "by_subscription" => _,
                   "total_seconds_available" => _
                 },
                 "timestamp" => _
               } = json_response(conn, 200)

        assert total >= 1
      end
    end

    defp insert_job(user_id, status, opts \\ []) do
      %Job{}
      |> Job.changeset(%{
        user_id: user_id,
        filename: opts[:filename] || "test.mp3",
        status: status,
        error: opts[:error]
      })
      |> Repo.insert!()
    end
  else
    # When admin routes are disabled, add a placeholder test
    @admin_enabled Application.compile_env(:poddyclip_backend, :admin_enabled)

    test "admin routes are disabled" do
      assert @admin_enabled == false
    end
  end
end
