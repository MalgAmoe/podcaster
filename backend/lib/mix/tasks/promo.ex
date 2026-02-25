defmodule Mix.Tasks.Promo do
  @moduledoc """
  Manage promotional offers for testing and production.

  ## Create a promo

      mix promo create NAME --bonus 1800 --max 5 --hours 24

  Options:
    --bonus   Bonus seconds to give new signups (required)
    --max     Maximum number of claims (required)
    --hours   How many hours the promo lasts from now (default: 24)

  ## List active promos

      mix promo list

  ## Deactivate a promo

      mix promo stop NAME

  ## Delete all promos (dev only)

      mix promo reset
  """

  use Mix.Task
  import Ecto.Query
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Billing.Promo

  @impl Mix.Task
  def run(args) do
    Mix.Task.run("app.start")

    case args do
      ["create" | rest] -> create(rest)
      ["list" | _] -> list()
      ["stop", name | _] -> stop(name)
      ["reset" | _] -> reset()
      _ -> Mix.shell().info(@moduledoc)
    end
  end

  defp create(args) do
    {opts, rest, _} =
      OptionParser.parse(args,
        strict: [bonus: :integer, max: :integer, hours: :integer]
      )

    name = List.first(rest)
    bonus = opts[:bonus]
    max_claims = opts[:max]
    hours = opts[:hours] || 24

    cond do
      is_nil(name) ->
        Mix.shell().error("Usage: mix promo create NAME --bonus SECONDS --max CLAIMS [--hours HOURS]")

      is_nil(bonus) or is_nil(max_claims) ->
        Mix.shell().error("--bonus and --max are required")

      true ->
        now = DateTime.utc_now() |> DateTime.truncate(:second)

        result =
          %Promo{}
          |> Promo.changeset(%{
            name: name,
            bonus_seconds: bonus,
            max_claims: max_claims,
            starts_at: now,
            expires_at: DateTime.add(now, hours, :hour),
            active: true
          })
          |> Repo.insert()

        case result do
          {:ok, promo} ->
            Mix.shell().info("""
            Promo created:
              Name:    #{promo.name}
              Bonus:   #{promo.bonus_seconds}s (#{div(promo.bonus_seconds, 60)}min)
              Claims:  0/#{promo.max_claims}
              Expires: #{promo.expires_at} (#{hours}h from now)
            """)

          {:error, changeset} ->
            errors = Ecto.Changeset.traverse_errors(changeset, fn {msg, _} -> msg end)
            Mix.shell().error("Failed: #{inspect(errors)}")
        end
    end
  end

  defp list do
    promos = Repo.all(from(p in Promo, order_by: [desc: p.inserted_at]))

    if promos == [] do
      Mix.shell().info("No promos found.")
    else
      now = DateTime.utc_now()

      Enum.each(promos, fn p ->
        status =
          cond do
            !p.active -> "INACTIVE"
            DateTime.compare(now, p.expires_at) == :gt -> "EXPIRED"
            DateTime.compare(now, p.starts_at) == :lt -> "FUTURE"
            p.claims_count >= p.max_claims -> "EXHAUSTED"
            true -> "ACTIVE"
          end

        Mix.shell().info(
          "  [#{status}] #{p.name} — #{p.bonus_seconds}s bonus, #{p.claims_count}/#{p.max_claims} claimed, expires #{p.expires_at}"
        )
      end)
    end
  end

  defp stop(name) do
    case Repo.get_by(Promo, name: name) do
      nil ->
        Mix.shell().error("Promo '#{name}' not found.")

      promo ->
        promo
        |> Ecto.Changeset.change(active: false)
        |> Repo.update!()

        Mix.shell().info("Promo '#{name}' deactivated.")
    end
  end

  defp reset do
    {count, _} = Repo.delete_all(Promo)
    Mix.shell().info("Deleted #{count} promo(s).")
  end
end
