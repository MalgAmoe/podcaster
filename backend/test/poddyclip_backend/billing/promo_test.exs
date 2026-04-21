defmodule PoddyclipBackend.Billing.PromoTest do
  use PoddyclipBackend.DataCase

  alias PoddyclipBackend.{Accounts, Billing}

  import PoddyclipBackend.AccountsFixtures
  import PoddyclipBackend.BillingFixtures

  describe "find_active_promo/0" do
    test "returns active promo within time window" do
      promo = promo_fixture()
      found = Billing.find_active_promo()
      assert found.id == promo.id
    end

    test "returns nil when no promos exist" do
      assert Billing.find_active_promo() == nil
    end

    test "ignores inactive promos" do
      _promo = promo_fixture(%{active: false})
      assert Billing.find_active_promo() == nil
    end

    test "ignores expired promos" do
      now = DateTime.utc_now() |> DateTime.truncate(:second)

      _promo =
        promo_fixture(%{
          starts_at: DateTime.add(now, -48, :hour),
          expires_at: DateTime.add(now, -1, :hour)
        })

      assert Billing.find_active_promo() == nil
    end

    test "ignores promos that haven't started yet" do
      now = DateTime.utc_now() |> DateTime.truncate(:second)

      _promo =
        promo_fixture(%{
          starts_at: DateTime.add(now, 1, :hour),
          expires_at: DateTime.add(now, 25, :hour)
        })

      assert Billing.find_active_promo() == nil
    end

    test "ignores exhausted promos" do
      _promo = promo_fixture(%{max_claims: 5, claims_count: 5})
      assert Billing.find_active_promo() == nil
    end
  end

  describe "claim_promo/1" do
    test "increments claims_count" do
      promo = promo_fixture(%{max_claims: 10})
      assert promo.claims_count == 0

      {:ok, updated} = Billing.claim_promo(promo)
      assert updated.claims_count == 1
    end

    test "returns error when exhausted" do
      promo = promo_fixture(%{max_claims: 1, claims_count: 1})
      assert {:error, :exhausted} = Billing.claim_promo(promo)
    end

    test "is atomic — cannot exceed max_claims" do
      promo = promo_fixture(%{max_claims: 1, claims_count: 0})

      {:ok, _} = Billing.claim_promo(promo)
      assert {:error, :exhausted} = Billing.claim_promo(promo)
    end
  end

  describe "registration with promo" do
    test "new user gets bonus seconds when promo is active" do
      _promo = promo_fixture(%{bonus_seconds: 1800})

      {:ok, user} = Accounts.register_user(valid_user_attributes())
      # 1800 (free plan) + 1800 (promo bonus) = 3600
      assert user.seconds_available == 3600
      assert user.seconds_allocated == 3600
    end

    test "new user gets normal seconds when no promo exists" do
      {:ok, user} = Accounts.register_user(valid_user_attributes())
      assert user.seconds_available == 1800
      assert user.seconds_allocated == 1800
    end

    test "promo claims_count increments on registration" do
      promo = promo_fixture(%{bonus_seconds: 1800})
      assert promo.claims_count == 0

      {:ok, _user} = Accounts.register_user(valid_user_attributes())

      updated = Repo.get!(Billing.Promo, promo.id)
      assert updated.claims_count == 1
    end

    test "promo stops applying after max_claims reached" do
      _promo = promo_fixture(%{bonus_seconds: 1800, max_claims: 1})

      # First signup gets the bonus
      {:ok, user1} = Accounts.register_user(valid_user_attributes())
      assert user1.seconds_available == 3600

      # Second signup gets normal amount
      {:ok, user2} = Accounts.register_user(valid_user_attributes())
      assert user2.seconds_available == 1800
    end

    test "user gets base seconds if promo exhausted between find and claim" do
      promo = promo_fixture(%{bonus_seconds: 1800, max_claims: 1})

      # Simulate another process claiming the last slot before our registration
      {:ok, _} = Billing.claim_promo(promo)

      # Promo is now exhausted — registration should still succeed with base seconds
      {:ok, user} = Accounts.register_user(valid_user_attributes())
      assert user.seconds_available == 1800
      assert user.seconds_allocated == 1800
    end

    test "promo claim is rolled back if user insert fails" do
      promo = promo_fixture(%{bonus_seconds: 1800, max_claims: 1})

      # Try to register with a duplicate email to trigger insert failure
      {:ok, _existing} = Accounts.register_user(valid_user_attributes(%{email: "dupe@test.com"}))

      # Promo was claimed once
      assert Repo.get!(Billing.Promo, promo.id).claims_count == 1

      # Attempt duplicate registration — should fail and rollback the claim
      {:error, _changeset} = Accounts.register_user(valid_user_attributes(%{email: "dupe@test.com"}))

      # Claims count should still be 1, not 2
      assert Repo.get!(Billing.Promo, promo.id).claims_count == 1
    end
  end
end
