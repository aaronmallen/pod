---
id: "0027"
title: Canonical Asset Valuation Chain
status: active
tags: [data-model, assets, pricing]
created: 2026-06-19
---

# ADR-0027: Canonical Asset Valuation Chain

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Every site that values held assets — the Assets ▸ Values tab, the net-worth tracker, net-worth snapshots, and
asset-value sort/aggregates — resolves a per-asset unit price through one canonical COALESCE chain, identical for
characters and corporations:

```text
unit_price = CASE WHEN is_blueprint_copy THEN 0
             ELSE COALESCE(
                ab.muta_price_isk,   -- MutaMarket per-item (abyssals), canonical
                mp.adjusted_price,   -- ESI estimated
                mp.average_price,    -- ESI estimated, OR zKill-filled gap value
                0)
             END
value = quantity * unit_price        -- abyssals are always quantity 1
```

MutaMarket per-item prices are canonical for abyssal (rolled) items; zKillboard fills ESI type-price gaps into
`market_prices.average_price`; both are layered into the single shared valuation join rather than computed
independently per site. Price provenance is tracked internally (`market_prices.source`) but not surfaced in the UI this
iteration.

## Context

Pod's asset valuation understated value and reported an incorrect Values-tab total:

1. ESI price gaps. `GET /markets/prices/` returns no price for the most expensive type-priced items (titans,
   supercarriers, faction/officer modules), so they valued at 0 ISK.
2. Abyssals valued by base hull. Abyssal modules are unique rolled instances. Accurate per-item MutaMarket prices were
   already fetched and stored (`abyssal_items.muta_price_isk`) and shown on the Abyssals tab, but every other valuation
   site ignored them and valued each abyssal at its base-type `market_prices` figure (often 0).
3. Page-window total. The Values tab summed only the currently-loaded keyset page, so its total diverged from true net
   worth.

The root cause of (1) and (2) being inconsistent across surfaces was that valuation logic was not centralized: the page
query, the `character_financials` view, and the as-of aggregates each carried their own price expression. Any new price
source had to be threaded through each site independently and could silently drift.

## Decision

Establish one canonical per-asset valuation chain, expressed in the shared `query_join_sql!` / unit-price macros in
`src/store/repo/assets.rs`, and mirror it at every other SQL valuation site so the same per-asset value is produced
everywhere.

- MutaMarket is canonical for abyssals. The valuation join LEFT JOINs the owner-scoped abyssal table on `item_id`; a
  non-null `muta_price_isk` wins over all ESI prices. Unlisted abyssals (null muta price) fall back through the normal
  chain.
- zKillboard fills ESI type-price gaps only. When the resolved ESI price for a held type is null/0, the MarketPrices job
  fetches zKill's per-type price and upserts it into `market_prices.average_price` tagged `source = 'zkill'`. zKill
  never overrides a non-zero ESI price.
- Price provenance is a column, not value-inferred. `market_prices` gains `source TEXT NOT NULL DEFAULT 'esi'` and
  `fetched_at`. The zKill refresh set is keyed off `source` (absent, OR `source='zkill'`, OR `source='esi'` with
  resolved price 0) so a zKill-filled `average_price` does not stop matching the gap check and go permanently stale; an
  ESI upsert rewrites `source='esi'`, reclaiming the row when ESI later prices the type.
- Char/corp parity by owner-parameterized macro. The valuation macros are parameterized by asset table + owner column;
  corp abyssal prices live in a dedicated `corporation_abyssal_items` table (mirroring the existing char/corp table
  split — `corporation_assets`, `corporation_blueprints`, etc.), and the macro is instantiated per owner with its own
  abyssal table.
- Totals are DB aggregates. The Values-tab total is computed over the full asset scope via the same valuation chain, not
  by summing a page, so it equals the tracker's net-worth asset value.

The `character_financials` view and the as-of aggregates are recreated/updated to carry the identical abyssal join and
chain, keeping all SQL sites in sync.

## Affected Areas

- `src/store/repo/assets.rs` — shared valuation macros (page query, render, as-of aggregates).
- `migrations/` — `market_prices` alter (`source` + `fetched_at`); recreate `character_financials` view; new
  `corporation_abyssal_items` table.
- `src/store/model/market_price.rs`, `src/store/repo/finance.rs` — model + `market_prices_upsert_many` write `source`.
- `src/sync/jobs/market_prices.rs` — zKill gap fallback folded in (no new `JobKind`).
- `src/clients/zkillboard.rs` — new `prices(type_id)` method.
- `src/sync/jobs/abyssals.rs` — extended to corporation subjects.
- `src/features/assets/values.rs`, `src/features/assets.rs` — full-set total.

## Consequences

### Positive

- A single price expression governs every valuation surface; new price sources are added in one place and propagate
  everywhere.
- Titans/supers/officer modules and accurately-rolled abyssals show real values; the Values-tab total equals net worth.
- Source-tagging enables a future canonical-source flip and provenance badges without a schema change.

### Negative

- Three valuation SQL sites (page query, `character_financials` view, as-of aggregate) must be kept consistent by hand;
  an acceptance test asserts the page query and the view agree.
- zKill adds throttled per-type requests to the MarketPrices job (bounded to the small ESI-gap set; failures non-fatal).
- A second abyssal table (`corporation_abyssal_items`) duplicates the char schema rather than generalizing one table.

## Future Work

- UI provenance badges / source indicators (out of scope this iteration).
- Making zKill canonical over non-zero ESI prices (gap-only today).
- `fetched_at`-driven max-age skip to further bound zKill request volume (default: re-fetch every cycle).
- A corporation financials view, if corp net worth grows beyond the current wallet-only snapshot + as-of asset
  aggregate.

## References

- Spec: rkqurypk (Canonical asset valuation — MutaMarket + zKillboard)
- [0008] (Assets Data Path), [0009] (Daily Net-Worth Snapshot)
- zKill prices API: <https://github.com/zKillboard/zKillboard/wiki/API-(Prices)>

[0008]: 0008-assets-data-path.md
[0009]: 0009-daily-net-worth-snapshot.md
