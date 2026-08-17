# Tactical 318: Seasonal Resource Opportunity

Status: planned 2026-08-17; ready for implementation

Parent: [`315`](315-authoritative-seasonal-calendar-and-squirrel-ecology.md)
Phase C

Topic: `seasons`

Topic: `habitat-driven-creature-ecology`

Topic: `wildlife-ecology-state-model`

## Instruction Synthesis

Make the authoritative calendar affect the five existing wildlife resource
strata through ordinary loaded opportunity. Preserve terrain-derived
potential and durable standing stock. Derive current accessibility and loaded
recovery from the calendar's local seasonal climate, then let only a
physically reached compatible feeding action consume accessible stock.

Calendar changes are sharp condition changes, not elapsed-time events. A
forward or backward date mutation may immediately alter accessibility and the
factor applied by future active recovery ticks, but it must not add, delete,
replay, or reconcile stock. Inactive cells remain frozen and first activation
does not integrate missed seasons.

This slice covers low herbaceous growth, woody browse, seeds/soft mast,
aquatic vegetation, and aquatic invertebrates. It re-accepts the existing
rabbit, deer, and mallard lifecycle/campaign evidence before squirrel demand
is introduced. It does not add squirrels, caches, seasonal reproduction,
weather, decay, unloaded ecology, a carrying-capacity estimator, or a new
resource stratum.

## Shared Pure Response

Add revisioned resource-response vocabulary to `mclone-season`, the same
dependency leaf that owns authoritative calendar and local-season
evaluation. A pure query consumes one resource kind, one
`EvaluatedLocalSeason`, and normalized local moisture and returns fixed-point
factors:

```text
SeasonalResourceOpportunity {
    accessibility,
    recovery,
}
```

Both factors use exact `0..=10_000` basis points at the server boundary.
`10_000` means the existing terrain opportunity; zero means no currently
available opportunity or production. Curves are continuous over local phase,
current temperature, moisture, snow tendency, and response strength. They do
not branch on a global Spring/Summer/Autumn/Winter label.

The first response must retain these product differences:

| Stratum | Accessibility | Loaded recovery |
|---|---|---|
| low herbaceous | cold and snow can conceal much of standing growth, without deleting it | strongest in warm/moist conditions and weak when cold, snow-covered, or dry |
| woody browse | remains relatively accessible through lean conditions | restrained thermal/moisture response with slower baseline recovery |
| seeds and soft mast | broad late-growth and decline-season surplus, reduced but nonzero lean-season access | broad continuous mast-production shoulder rather than a boundary event |
| aquatic vegetation | cold suppresses access without inventing or deleting aquatic terrain stock | warm/moist response, still exactly zero where terrain potential is zero |
| aquatic invertebrates | activity falls in cold water and dry climate | temperature/moisture activity response while physical water evidence remains mandatory |

When the calendar policy is disabled, return exact neutral factors so retained
profiles and existing feature-off behavior are unchanged. Tropical latitude,
opposite hemispheres, elevation, and wet/dry annual climate must flow through
the same `EvaluatedLocalSeason` input rather than special cases.

## Authoritative Local Sampling

`mclone-server` owns a dimension-local seasonal resource sampler. For
`mclone-overworld-v1`, sample the production terrain field at the center of
the 64-by-64 resource cell, use the dimension's topology-aware effective
latitude, annual temperature/moisture, and terrain surface altitude, and
evaluate the current authoritative orbital sample. Retained profiles have no
calendar and therefore produce neutral opportunity.

The sampler is a constant-time query over an already bounded active cell. It
must not enumerate climate cells, load terrain, use renderer settings, or
store another advancing clock. Recovery and intake within one cell consume
the same cell-center climate contract.

## Ledger Semantics

Keep persisted `potential` and `available` as terrain potential and standing
stock. Do not replace standing stock when opportunity changes. For a snapshot:

```text
effective_accessible_units =
    floor(standing_stock * accessibility_basis_points / 10_000)
```

A reached bite is capped by current effective accessible units and then
subtracts only the consumed amount from standing stock. Recovery retains the
existing per-stratum base cadence but multiplies each active recovery
numerator by the current recovery factor using a deterministic integer
remainder. Standing stock never exceeds terrain potential. Resampling changed
terrain may still lower potential and clamp stock under the existing terrain
revision rule; a calendar change may not.

Advance the wildlife-resource rule revision and reject incompatible internal
records explicitly. Persist standing stock, recovery remainder, recovered
units, and per-consumer transfers; do not persist derived current climate or
opportunity beside the authoritative civil clock.

Extend snapshots/reports with:

- terrain potential;
- standing stock;
- accessibility basis points;
- effective accessible units;
- recovery-factor basis points;
- cumulative recovered units; and
- the existing per-consumer cumulative transfers.

## Active And Discontinuous Time

Only resource cells overlapping the current entity-ticking domain execute a
recovery step. Inactive cells preserve stock and integer remainder exactly.
The closed-domain accelerated ecology path must consume the same civil clock,
local sampler, response curve, and resource step as the full authoritative
tick path.

Direct forward/backward date changes and successful sleep do no resource work
themselves. The next executed lifecycle cadence observes the new conditions
once. Repeated date toggles therefore cannot create stock, erase stock, or
alter cumulative recovery/consumption receipts.

## Validation

- pure response landmarks for all five strata, curve bounds, continuity,
  neutral disabled policy, northern/southern inversion, tropical weakness,
  wet/dry contrast, and altitude cooling;
- snapshot arithmetic at zero, partial, and full accessibility;
- reached intake capped by effective accessible units while incompatible or
  unavailable physical terrain still yields zero;
- active recovery uses the response factor, conserves bounds and integer
  remainder, and never invents a zero-potential resource;
- forward and backward calendar jumps change only derived opportunity;
- repeated date toggles leave potential, stock, remainder, recovered totals,
  and consumer transfers unchanged until one real lifecycle tick executes;
- inactive cells and unload/reload preserve exact state with no catch-up;
- save/reopen restores the same durable stock and recomputes opportunity from
  the restored calendar;
- full and accelerated closed-domain paths remain exact;
- existing rabbit/deer/mallard focused suites and the accepted multi-seed
  resource/population windows pass at the new rule revision; and
- `cargo check --workspace`, Wasm build/typecheck, and thin-adapter purity
  remain green.

## Review Gate C

Produce one deterministic report comparing at least northern/southern
temperate, tropical, high/cold, and wet/dry cells at selected orbital dates.
It must make the causal potential/stock/access/recovery distinctions readable.
Also produce forward/backward-jump and inactive-freeze receipts plus exact
full/accelerated equivalence.

Accept the gate only if resource curves differ for climate and terrain
reasons, no calendar operation directly mutates stock, exact conservation
holds, and the established rabbit/deer/mallard lifecycle remains viable
without a target population correction. Do not begin squirrel tuning until
this evidence is recorded.

## Completion Checklist

- [ ] Add and test the pure fixed-point five-stratum response.
- [ ] Add topology-aware authoritative local sampling in `mclone-server`.
- [ ] Apply accessibility only to physically reached intake.
- [ ] Apply recovery only to loaded active-cell production.
- [ ] Extend persistence revision, snapshots, reports, and invariants.
- [ ] Prove sharp date jumps and inactive time do no resource work.
- [ ] Re-accept full/accelerated and rabbit/deer/mallard campaigns.
- [ ] Record Review Gate C evidence in Tactical 315 and living topics.
