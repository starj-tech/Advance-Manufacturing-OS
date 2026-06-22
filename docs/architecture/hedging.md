# Global Supply Chain Hedging

> Status: skeleton with deterministic moving-average heuristic + Supabase
> schema. ML forecast + provider adapters land in PR #7.

## Goals

- Treat each material as **commodity exposure**: an EV battery cell is
  ~60% lithium + 20% cobalt + 15% copper.
- Continuously ingest price feeds from LME / CME / ICE / SHFE.
- Generate actionable signals: "Buy 6 months of HRC steel now; spot is
  16% below the 90-day average. Estimated savings: $X".
- Let the manager accept / dismiss inline.

## Components

`crates/aether-hedging`:

- `Commodity` enum — Steel HRC, Aluminum, Copper, Polypropylene, Brent,
  Natural gas, Lithium, Cobalt. Each carries a ticker + unit.
- `MaterialExposure` — `(material_sku, commodity, weight)` triplet,
  weight in `[0, 1]`. A material can have multiple exposures.
- `CommodityFeed` trait — `latest(commodities)` and `history(commodity, range)`.
- `recommend(commodity, history, window, now)` — pure function returning
  `HedgeRecommendation { spot, sma, action, savings_pct, rationale }`.
- `HedgeAction::BuyNow { cover_months }`, `Wait`, `Hold`.

## Heuristic (skeleton)

Simple SMA: if `spot < SMA - 10%` → BuyNow(6). If `spot > SMA + 10%` → Wait.
Else Hold. Unit-tested.

PR #7 replaces this with an LSTM forecast + Monte Carlo simulation
behind the same trait surface; consumers don't change.

## Schema

`supabase/migrations/0005_commodities.sql`:

- `commodities` — ticker dictionary
- `commodity_prices` — TimescaleDB hypertable, retention 5 years
- `commodity_prices_daily` — continuous aggregate (avg/min/max/close)
- `material_commodity_map` — per-tenant exposure weights
- `hedge_recommendations` — generated suggestions + acceptance/dismissal

## Feed ingest

`supabase/functions/commodity-feed`: cron-driven (every 5m). Pulls from
configured providers, normalizes to USD via `exchange_rates`, and
upserts into `commodity_prices`. Real provider adapters and rate-limit
accounting ship with PR #7.
