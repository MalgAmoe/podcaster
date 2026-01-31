defmodule Mix.Tasks.DetectAbuse do
  @moduledoc """
  Detects potential email abuse patterns for Sybil attack detection.

  Outputs a JSON report that can be:
  - Reviewed manually
  - Fed to an LLM for analysis help

  Usage:
    mix detect_abuse > abuse_report.json

  No data is stored - this is a read-only detection script.
  """

  use Mix.Task
  import Ecto.Query
  alias PoddyclipBackend.Repo
  alias PoddyclipBackend.Accounts.User
  alias PoddyclipBackend.Processing.Job

  @shortdoc "Detect potential email abuse patterns"

  def run(_args) do
    Mix.Task.run("app.start")

    report = %{
      generated_at: DateTime.utc_now(),

      # PATTERN 1: Plus aliases
      # Many email providers support user+tag@domain.com as aliases
      # Example: attacker+1@gmail.com, attacker+2@gmail.com
      # Groups by base email (before +) to find accounts sharing same base
      plus_aliases: find_plus_aliases(),

      # PATTERN 2: Gmail dot variants
      # Gmail ignores dots: u.s.e.r@gmail.com = user@gmail.com
      # Also catches +aliases for Gmail specifically
      # Example: john.doe@gmail.com, johndoe@gmail.com, j.ohndoe+test@gmail.com
      gmail_dot_variants: find_gmail_dot_variants(),

      # PATTERN 3: Domain clusters
      # Someone with their own domain can create infinite emails (catch-all)
      # Flags non-major domains with 4+ accounts
      # Example: a@attacker.com, b@attacker.com, c@attacker.com
      domain_clusters: find_domain_clusters(),

      # PATTERN 4: Heavy free-tier usage
      # Free tier gets 15 mins. Users who have processed more than that
      # while still on free plan might be gaming (got refunds, exploits, etc)
      # Calculated by summing estimated_minutes from completed jobs
      free_tier_heavy_users: find_free_tier_heavy_users()
    }

    IO.puts(Jason.encode!(report, pretty: true))
  end

  # Find emails with + that share the same base address.
  #
  # Example match:
  #   user+spotify@domain.com
  #   user+newsletter@domain.com
  # → Both normalize to user@domain.com
  #
  # Returns: List of groups where multiple accounts share a base email.
  # Only includes bases with 2+ accounts.
  defp find_plus_aliases do
    from(u in User,
      where: like(u.email, "%+%@%"),
      select: %{
        id: u.id,
        email: u.email,
        # Reconstruct base: take part before + and add domain
        base: fragment("split_part(?, '+', 1) || '@' || split_part(?, '@', 2)", u.email, u.email),
        created: u.inserted_at,
        minutes_available: u.minutes_available
      }
    )
    |> Repo.all()
    |> Enum.group_by(& &1.base)
    |> Enum.filter(fn {_base, users} -> length(users) > 1 end)
    |> Enum.map(fn {base, users} ->
      %{
        base_email: base,
        count: length(users),
        accounts: Enum.map(users, &Map.drop(&1, [:base]))
      }
    end)
  end

  # Find Gmail addresses that normalize to the same base.
  #
  # Gmail quirks:
  # - Dots are ignored: j.o.h.n@gmail.com = john@gmail.com
  # - Plus aliases work: john+spam@gmail.com = john@gmail.com
  # - googlemail.com = gmail.com
  #
  # Example match:
  #   john.doe@gmail.com
  #   johndoe@gmail.com
  #   john.doe+test@googlemail.com
  # → All normalize to johndoe@gmail.com
  #
  # Returns: List of groups where multiple Gmail accounts normalize to same base.
  # Only includes bases with 2+ accounts.
  defp find_gmail_dot_variants do
    from(u in User,
      where: like(u.email, "%@gmail.com") or like(u.email, "%@googlemail.com"),
      select: %{
        id: u.id,
        email: u.email,
        created: u.inserted_at,
        minutes_available: u.minutes_available
      }
    )
    |> Repo.all()
    |> Enum.group_by(fn u ->
      # Normalize: lowercase, remove dots, remove +alias
      u.email
      |> String.downcase()
      |> String.split("@")
      |> hd()
      |> String.replace(".", "")
      |> String.split("+")
      |> hd()
    end)
    |> Enum.filter(fn {_base, users} -> length(users) > 1 end)
    |> Enum.map(fn {base, users} ->
      %{
        normalized_base: "#{base}@gmail.com",
        count: length(users),
        accounts: users
      }
    end)
  end

  # Find non-major domains with many accounts.
  #
  # If someone owns their domain, they can create unlimited emails via catch-all.
  # Major providers (gmail, outlook, etc.) are excluded since multiple
  # legitimate users share those domains.
  #
  # Example match:
  #   alice@suspiciousdomain.com
  #   bob@suspiciousdomain.com
  #   charlie@suspiciousdomain.com
  #   dave@suspiciousdomain.com
  # → 4 accounts on same non-major domain is suspicious
  #
  # Returns: List of domains with 4+ accounts (excluding major providers)
  defp find_domain_clusters do
    # These domains are expected to have many users - don't flag them
    major_domains = ~w[
      gmail.com googlemail.com
      outlook.com hotmail.com live.com
      yahoo.com yahoo.fr yahoo.co.uk
      icloud.com me.com mac.com
      protonmail.com proton.me pm.me
      aol.com
      mail.com
      zoho.com
      yandex.com yandex.ru
      gmx.com gmx.net
      fastmail.com
      tutanota.com
    ]

    from(u in User,
      select: %{
        id: u.id,
        email: u.email,
        created: u.inserted_at,
        minutes_available: u.minutes_available
      }
    )
    |> Repo.all()
    |> Enum.group_by(fn u ->
      u.email |> String.downcase() |> String.split("@") |> List.last()
    end)
    |> Enum.filter(fn {domain, users} ->
      domain not in major_domains and length(users) >= 4
    end)
    |> Enum.map(fn {domain, users} ->
      %{
        domain: domain,
        count: length(users),
        accounts: users
      }
    end)
    |> Enum.sort_by(& &1.count, :desc)
  end

  # Find free-tier users who have used more minutes than the free allowance.
  #
  # Free tier gives 15 minutes. If a user on free plan has completed jobs
  # totaling more than 15 estimated_minutes, something is off:
  # - They got refunds/credits
  # - They exploited a bug
  # - They're part of a Sybil cluster rotating through accounts
  #
  # Sums estimated_minutes from completed jobs per user.
  #
  # Returns: List of free-tier users with total_minutes_used > 15
  defp find_free_tier_heavy_users do
    # Get all users on free plan
    free_users =
      from(u in User,
        join: p in assoc(u, :plan),
        where: p.name == "free",
        select: %{id: u.id, email: u.email, minutes_available: u.minutes_available, created: u.inserted_at}
      )
      |> Repo.all()

    free_user_ids = Enum.map(free_users, & &1.id)

    # Sum estimated_minutes from completed jobs per user
    usage_by_user =
      from(j in Job,
        where: j.user_id in ^free_user_ids and j.status == :completed,
        group_by: j.user_id,
        select: {j.user_id, sum(j.estimated_minutes)}
      )
      |> Repo.all()
      |> Map.new()

    # Find users who've used more than free tier allows (15 min)
    free_users
    |> Enum.map(fn user ->
      total_used = Map.get(usage_by_user, user.id, 0) || 0
      Map.put(user, :total_minutes_used, total_used)
    end)
    |> Enum.filter(fn user -> user.total_minutes_used > 15 end)
    |> Enum.sort_by(& &1.total_minutes_used, :desc)
  end
end
