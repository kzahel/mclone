# Structure Catalogue Product

Topic: `structure-catalogue-product`

Status: **product/growth/economy vision recorded 2026-07-21; brainstorm-stage.
This is not an accepted implementation direction and reserves no tactical. It
records where the public build catalogue could go once the read-only Structure
Lab catalogue exists, and it deliberately does not reopen or weaken
[`structure-lab.md`](structure-lab.md)'s read-only first proof. Nothing here is
a commitment to build; it is a captured north-star so the near-term
architecture is not chosen in a way that forecloses it.**

Last reconciled: **2026-07-21**.

## Scope

This topic owns the *product, growth, and economy* thesis for turning the
Structure Lab catalogue into a public "better GrabCraft" that also serves as a
zero-install acquisition funnel into Mclone. It covers the differentiators, the
value features that follow from owning an engine and an AI authoring pipeline,
the growth/distribution mechanics, and the credit/token economy that funds
bespoke AI authoring.

It does **not** own:

- the authoring pipeline, DSL, canonical-JSON drift gates, baked-mesh
  compilation, or the read-only catalogue contract — those stay with
  [`structure-lab.md`](structure-lab.md);
- the play/warm-handoff machinery that makes "walk into a build" possible —
  that reuses [`embedded-worlds.md`](embedded-worlds.md) and
  [`web-scene-host-adoption.md`](web-scene-host-adoption.md);
- structure/settlement content authoring — that stays with
  [`structure-lab.md`](structure-lab.md) and
  [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md); and
- the true cross-chunk structure lifecycle (`FS-04`) needed for the richer
  "walk out of the build into a generated world" experience.

The Structure Lab purpose it builds on is precise: **the lab exists to let AI
agents author buildings.** This topic asks what public product that authoring
capability, plus a web-capable engine, makes possible.

## The Reframe: A Generator, Not A Library

GrabCraft, PlanetMinecraft, and similar sites are *catalogues* whose moat is a
large pile of human uploads. We should not try to out-library them. Owning an
engine and AI authoring lets us make library size irrelevant:

- **We are a generator that keeps a greatest-hits gallery**, not a fixed
  library. A prompt ("cozy A-frame with a sleeping loft") yields a build the
  visitor can walk into, in the browser, with no download.
- **Walkable is the unfair advantage.** No screenshot site can let you step
  inside and play the build in the same tab. Every build page is a playable
  demo of the actual game.
- **The catalogue fills itself** from the exhaust of on-demand authoring rather
  than from an upload queue.

Everything below serves that inversion.

## Product Pillars (Value)

These follow directly from owning the engine and a deterministic semantic
record. Several are things a screenshot catalogue structurally cannot do.

- **Walk-through / step inside.** Every build page can drop the visitor into
  the real engine for a first-person tour. Try-before-you-build.
- **Build-along mode — honestly scoped.** Because the structure is semantic,
  the engine can ghost the next block(s), track progress, and let a player
  follow the build *inside Mclone*. This is **not** universal: in creative,
  pasting the whole structure wins and build-along is pure friction. Its real
  homes are **survival** (you cannot paste; gathering and placing is the game),
  **survival multiplayer servers** (pasting is banned, so a native guided
  blueprint is the only sanctioned path — and those communities are a
  distribution channel), and **onboarding** (a beginner following a guided
  build is learning to play). Offer both paste and build-along, routed by
  intent; do not pretend build-along beats paste.
- **Guides that are correct and free.** Layer slider, exploded axonometric,
  cutaway, and per-layer material *delta* ("this course: +14 oak planks, +3
  glass") are generated deterministically from the semantic record — never
  hand-authored, never wrong.
- **Bill of materials → gather list.** Total and per-layer counts, grouped by
  acquisition (mine/craft/trade), with a survival-cost tier ("cheap first-night
  house"). Later: fill a creative hotbar with exactly this.
- **Native remix and variation.** "Bigger," "spruce instead of oak," "add a
  second floor," "snowy." The DSL already has material themes and bounded
  family variants, so one build is really a family. Deterministic variations
  (theme swaps) are effectively free.
- **Compose into a world.** Snap several builds onto a plot (the DSL speaks
  sockets/paths), then jump in and play the whole hamlet — single builds become
  a settlement builder and a reason to spawn a world.
- **Real-engine context for free.** Day/night preview, biome backdrop swap, a
  figure for scale, golden-hour lighting — all native because it is an actual
  renderer.
- **Survival-friendliness metadata.** Filter by buildable-in-survival, material
  rarity, and estimated time.

## Growth And Distribution

The funnel *is* the engine: browse → play has no download cliff.

- **Programmatic SEO is the primary lever.** Every generated build is a landing
  page for a long-tail query, and each page is also a one-click playable demo.
  GrabCraft is capped by upload rate; we are capped by compute. The build is
  the bait, Mclone is the hook, and we can mint bait on demand.
- **Zero-install "Play" on every page**, because the web engine already runs.
- **Shareable deep links** into a build, a walk-through, or a "spawn into this
  world" — Discord/Reddit/TikTok-native, each share a demo link into the
  engine.
- **Auto-generated video/GIF/OG cards.** The Asset Lab already does
  deterministic Playwright capture; reuse it to emit a turntable + walk-through
  clip per build. Free social assets at scale.
- **Embeddable "walk this build" iframe** places the engine on other MC
  blogs/wikis — parasitic distribution.
- **Request queue → agents fill it.** People request builds; agents author
  them. Requests are free content, engagement, and fresh SEO pages.
- **Cadence and community loops** — build-of-the-day, seasonal drops, "beat
  this base" challenges — all agent-fed.

## The Prompt-Iteration Economy

This is the substantive new decision this topic records. The credit/token is
metered against the one thing with real marginal cost — **AI authoring
inference** — while discovery, play, and reuse stay free.

The iteration loop *is* the paid product: "the tower looks weird," "fix the
door," "make the windows bigger." That conversational refinement is genuinely
expensive (each step is an agent pass plus a re-bake) and genuinely valuable,
so it is the honest place to put the meter.

### Why here and not on paste/play

- **Discovery and the demo stay frictionless.** Browsing, walking in, playing,
  and pasting existing builds are cheap to serve (static meshes + engine) and
  free to the user, protecting the growth funnel.
- **Credits buy compute, not power.** No pay-to-win, no gate on the cool thing.
  Users already accept "generation costs" from Midjourney/GPT-style tools.
- **The meter solves catalogue dilution.** The risk of infinite free AI builds
  burying human-directed ones disappears once nobody floods the catalogue for
  free; quality is gated by someone caring enough to spend or earn.

### Cost ladder (price maps to inference intensity)

- **Free:** play, paste, re-render (new camera / time of day), and
  deterministic variations (theme/material swaps — a param change and a re-bake,
  not inference). Free variations are good funnel sugar.
- **Cheap:** local refinements ("fix the door," "bigger windows") — a targeted
  edit to existing DSL plus a partial re-bake. Keep these cheap on purpose so
  exploration feels safe.
- **Expensive:** from-scratch generation — a full authoring pass plus bake.

The ladder also nudges reuse: iterating on an existing catalogue build is
cheaper than starting cold.

### The closed creator loop

Iterate (spend) → publish → others walk in and build it → **you earn credits
from that usage** → fund your next iteration. Consequences:

- **Good curators self-fund**; the community's usage pays for their next build.
- **Waste self-throttles**; junk burns the author's own credits with nothing
  back.
- **The creative act is taste, not construction.** Because agents do the
  building, the credited creator is whoever *directed* the build — a no-skill
  creator on-ramp, and a genuinely novel wedge that only exists because we have
  AI authoring underneath. Reputation ("directed by you", played/built counts)
  is often a stronger motivator than currency and is free to ship.

### Real money, soft currency, and guards

- **Real money enters at exactly one honest place:** buying compute for bespoke
  iteration, with a free taste tier and a usage-funded path so the passionate
  and the popular rarely pay cash.
- **Prefer soft currency + reputation early.** Earned credits and rep spent on
  cosmetics, palette unlocks, featured placement, or generation quota avoid the
  fraud/legal/moderation swamp of real-money-per-use.
- **Two failure modes that must be designed for:**
  - *Credit anxiety kills exploration.* Show cost before the action and keep
    refinements cheap and predictable, or people watch the counter instead of
    playing with ideas.
  - *Earn→compute is a direct cost leak.* Because credits buy inference (a real
    cost), "earn when used" is farmable by sock-puppet usage. The *earn* side
    needs per-real-user dedup/rate-limiting (or earned credits discounted vs.
    bought); the *spend* side is safe.

## Strategic Thesis (Moat)

The moat is the *integration*, not any single piece: an in-browser engine that
plays, an AI pipeline that authors on demand, and a deterministic semantic
format that makes guides/BoM/build-along correct and free. A competitor can
bolt an LLM onto "generate a schematic," but reproducing walkable + build-along
+ perfect guides + a game to funnel into is the hard part.

The compounding advantage is **usage signal GrabCraft cannot get**: not upload
popularity but "did people actually walk in, remix, and build this to
completion." That signal feeds back into what agents author next, so catalogue
quality compounds with use.

## Relationship And Guardrails

- **Do not reopen the read-only first proof.** `structure-lab.md`'s first
  product stays read-only static baked meshes. "Play this build" is an
  additive, opt-in, full-engine transition on click — never a per-card WebGL
  context — so it respects the one-canvas rule.
- **Play reuses landed systems.** The walk-in/warm-handoff path is composition
  of `web-scene-host-adoption.md` (the web client already plays worlds) and
  `embedded-worlds.md` (warm handoff, catalog-world destinations,
  supported-arrival). The standalone Structure Lab galleries already stamp a
  structure into a small persistent world. The MVP of "jump in and play a build"
  needs none of the cross-chunk structure lifecycle; only the richer "walk out
  into a generated overworld" version needs `FS-04`.
- **In-first-person editing stays deferred.** It conflicts with the source-first
  DSL authority (editing produces blocks, not DSL — a forbidden second authoring
  surface) and is gated on a capture-to-source format (an open question in
  `starter-farmstead-settlement.md`). It is a later north-star, not a near
  phase.
- **The economy must never gate the top of the funnel.** Metering lives on
  bespoke inference only. The moment discovery or the playable demo needs
  credits, the best growth asset has been traded for a mediocre revenue one.
- **The economy serves acquisition/retention, not vice versa.** Do not let the
  token layer become the product.

## Open Questions

- What is the smallest flagship worth building first: walk-through +
  build-along (the demo that sells the game), the programmatic-SEO page engine
  (distribution), or prompt-to-playable-world (the "wow")?
- What are the concrete free-tier size, refinement vs. generation prices, and
  earn rates, and where (if anywhere) does real money first appear?
- What sybil/dedup rule makes "earn credits when used" safe given earned credits
  buy real compute?
- How fast must the iterate→re-bake loop feel before conversational refinement
  is delightful rather than a chore, and what latency budget does that imply?
- Should earned and purchased credits be fungible, or should earned credits be
  discounted/limited to a subset of sinks?
- What is the minimum capture-to-source path that would let in-engine edits
  round-trip to canonical DSL without creating a second authoring surface?
- Which build-along context (survival single-player, survival server, or
  onboarding) is the best first proof that build-along beats an alt-tabbed
  blueprint?

## Related

- [`structure-lab.md`](structure-lab.md)
- [`starter-farmstead-settlement.md`](starter-farmstead-settlement.md)
- [`embedded-worlds.md`](embedded-worlds.md)
- [`web-scene-host-adoption.md`](web-scene-host-adoption.md)
- [`animal-catalogue.md`](animal-catalogue.md)
- [`asset-pack-profiles.md`](asset-pack-profiles.md)
- [`../native-web.md`](../native-web.md)
