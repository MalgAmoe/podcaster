# Polar Billing Setup Guide

This guide explains how to configure [Polar.sh](https://polar.sh) for subscription billing.

## Quick Start

1. Create a Polar account and organization
2. Create a subscription product ($15/month)
3. Copy the product ID and set `POLAR_PRO_PRODUCT_ID`
4. Set up webhook endpoint and copy secret to `POLAR_WEBHOOK_SECRET`
5. Run seeds: `mix run priv/repo/seeds.exs`

---

## Environment Variables

Add these to your `.env` or `.env.local` file:

```bash
# Required
POLAR_WEBHOOK_SECRET=whsec_xxx      # From Polar webhook settings
POLAR_PRO_PRODUCT_ID=prod_xxx       # From Polar product page

# Optional (have defaults)
POLAR_ORGANIZATION=munchy-cow       # Your Polar org slug (default: munchy-cow)
BASE_URL=https://munchycow.com      # For success redirect URLs
```

---

## Step-by-Step Setup

### 1. Create Polar Account & Organization

1. Go to [polar.sh](https://polar.sh) and sign up
2. Create an organization - the slug (e.g., `munchy-cow`) becomes your `POLAR_ORGANIZATION`

### 2. Create Subscription Product

1. In your Polar dashboard, go to **Products**
2. Click **Create Product**
3. Configure:
   - **Name**: "Pro" (or your plan name)
   - **Type**: Subscription
   - **Price**: $15/month
   - **Description**: "15 hours of audio processing per month"
4. Save the product

### 3. Get Product ID

1. Go to **Products** in dashboard
2. Find your Pro product
3. Click the **context menu** (three dots) next to the product
4. Select **Copy Product ID**
5. Set this as `POLAR_PRO_PRODUCT_ID` in your environment

### 4. Configure Webhook

1. In Polar dashboard, go to **Settings > Webhooks**
2. Click **Add Endpoint**
3. Configure:
   - **URL**: `https://yourdomain.com/api/webhooks/polar`
   - **Format**: Raw (JSON)
   - **Secret**: Generate or set your own
4. Select these events:
   - `subscription.active`
   - `subscription.updated`
   - `subscription.canceled`
   - `subscription.revoked`
5. Copy the webhook secret (starts with `whsec_`)
6. Set as `POLAR_WEBHOOK_SECRET` in your environment

### 5. Update Database

Run seeds to update the Pro plan with the product ID:

```bash
POLAR_PRO_PRODUCT_ID=your_product_id mix run priv/repo/seeds.exs
```

Or update directly in database:

```sql
UPDATE plans
SET polar_product_id = 'your_product_id_here'
WHERE name = 'pro';
```

---

## How It Works

### Checkout Flow

1. User clicks "Upgrade to Pro" on `/account`
2. Redirects to Polar checkout: `https://polar.sh/{org}/checkout/{product_id}?email=user@example.com&success_url=...`
3. User completes payment on Polar
4. Polar sends `subscription.active` webhook
5. App upgrades user to Pro plan
6. User redirected to `/account?upgraded=true`

### Checkout URL Format

The app generates checkout URLs like:
```
https://polar.sh/munchy-cow/checkout/prod_xxx?email=user@example.com&success_url=https://munchycow.com/account?upgraded=true
```

You can also prefill customer name with `customer_name` parameter.

### Customer Portal

Pro users can manage their subscription via the Polar customer portal:
```
https://polar.sh/{org}/portal?customer_id={polar_customer_id}
```

This is automatically linked from the account page once the user has a `polar_customer_id`.

---

## Webhook Events

| Event | When | App Action |
|-------|------|------------|
| `subscription.active` | Payment succeeds | Upgrade user to Pro, set minutes, store customer ID |
| `subscription.updated` | Any change | Update subscription details, period dates |
| `subscription.canceled` | User cancels | Mark as cancelled, keep access until period end |
| `subscription.revoked` | Immediate cancellation | Downgrade to free immediately |

### Subscription Statuses

- **active**: Subscription is paid and active
- **cancelled**: User cancelled but has access until `current_period_ends_at`
- **past_due**: Payment failed (handled via `subscription.updated`)
- **revoked**: Immediate cancellation, no access

---

## Webhook Security

Polar uses [Standard Webhooks](https://www.standardwebhooks.com/) specification:

**Headers sent:**
- `webhook-id`: Unique event identifier
- `webhook-timestamp`: Unix timestamp
- `webhook-signature`: HMAC-SHA256 signature (format: `v1,{base64_signature}`)

**Signature verification:**
1. Message: `{webhook_id}.{webhook_timestamp}.{body}`
2. Sign with base64-decoded secret (strip `whsec_` prefix)
3. Compare signatures (timing-safe)
4. Reject if timestamp > 5 minutes old (replay protection)

The app handles this in `PoddyclipBackend.Polar.verify_signature/3`.

---

## Testing

### Local Development with ngrok

1. Install [ngrok](https://ngrok.com/)
2. Run: `ngrok http 4000`
3. Use ngrok URL for webhook endpoint: `https://xxx.ngrok.io/api/webhooks/polar`

### Sandbox/Test Mode

Polar has sandbox mode for testing:
- Test card: `4242 4242 4242 4242`
- Any future expiry date
- Any CVC

### Verify Webhook Delivery

1. Go to **Settings > Webhooks** in Polar dashboard
2. Click on your endpoint
3. View **Delivery History** to see sent webhooks and responses

---

## Troubleshooting

### Upgrade Button Shows "Billing not configured"

**Cause**: `polar_product_id` not set on Pro plan

**Fix**:
```bash
POLAR_PRO_PRODUCT_ID=your_id mix run priv/repo/seeds.exs
```

Verify:
```sql
SELECT name, polar_product_id FROM plans WHERE name = 'pro';
```

### Webhook Returns 401 Unauthorized

**Cause**: Webhook secret mismatch

**Fix**: Re-copy the secret from Polar dashboard, ensure `whsec_` prefix is included

### Webhook Returns 400 Bad Request

**Cause**: Timestamp too old or invalid signature

**Check**:
- Server clock is synchronized (NTP)
- Secret is correctly base64 encoded
- Raw body is being passed (not parsed JSON)

### User Not Upgraded After Payment

1. Check Polar dashboard: **Webhooks > Delivery History**
2. Check Phoenix logs for webhook processing errors
3. Verify webhook URL is publicly accessible
4. Check idempotency: same event may have been processed already

### "Contact support to manage subscription"

**Cause**: User has no `polar_customer_id` (subscription created before webhook integration)

**Fix**: Manually set `polar_customer_id` from Polar dashboard, or wait for next subscription event

---

## API Reference

### Key Functions

```elixir
# Generate checkout URL for upgrade
Polar.checkout_url(user, pro_plan)
# => "https://polar.sh/munchy-cow/checkout/prod_xxx?email=...&success_url=..."

# Generate customer portal URL
Polar.customer_portal_url(user)
# => "https://polar.sh/munchy-cow/portal?customer_id=cus_xxx"

# Verify webhook signature
Polar.verify_signature(raw_body, headers, secret)
# => {:ok, payload} | {:error, reason}
```

### Database Fields (users table)

| Field | Description |
|-------|-------------|
| `plan_id` | Foreign key to plans table |
| `minutes_available` | Remaining processing minutes |
| `polar_customer_id` | Polar customer ID for portal access |
| `polar_subscription_id` | Current subscription ID |
| `subscription_status` | active, cancelled, past_due, revoked |
| `current_period_ends_at` | When current billing period ends |

---

## Sources

- [Polar Webhook Setup](https://polar.sh/docs/integrate/webhooks/endpoints)
- [Polar Webhook Events](https://polar.sh/docs/integrate/webhooks/events)
- [Polar Checkout Links](https://polar.sh/docs/features/checkout/links)
- [Standard Webhooks Spec](https://www.standardwebhooks.com/)
