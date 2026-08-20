# Structure Catalogue Product

Topic: `structure-catalogue-product`

Status: **product/growth/economy vision recorded 2026-07-21 and expanded
2026-08-20 into a cross-ecosystem structure-publishing thesis and automated
construction-reveal acquisition system; brainstorm-stage. This is not an
accepted implementation direction and reserves no tactical. It records where
the public build catalogue could go once the read-only Structure Lab catalogue
exists, and it deliberately does not reopen or weaken
[`structure-lab.md`](structure-lab.md)'s read-only first proof. Nothing here is
a commitment to build; it is a captured north-star so the near-term
architecture is not chosen in a way that forecloses it.**

Last reconciled: **2026-08-20**.

## Scope

This topic owns the *product, growth, creator, and economy* thesis for turning
the Structure Lab catalogue into a cross-ecosystem structure-publishing
platform. Minecraft Java and Bedrock users may be complete, respected
customers of the catalogue rather than leads waiting to be converted. Mclone
is the most interactive supported destination and a zero-install option, not a
forced handoff. The topic covers cross-target publication, co-branded creator
storefronts, the licensed human-authored quality corpus, growth/distribution,
the deterministic build-reel compiler, and the credit economy that funds
bespoke AI authoring.

It does **not** own:

- the authoring pipeline, DSL, canonical-JSON drift gates, baked-mesh
  compilation, or the read-only catalogue contract — those stay with
  [`structure-lab.md`](structure-lab.md);
- concrete Java/Bedrock file encoders, target block-state mappings, add-on
  packaging, compatibility testing, or commercial approval — this topic
  records their product requirement but not their implementation owner;
- concrete video rendering, encoding, audio mixing, render-farm operation,
  native-Minecraft capture automation, or social-network publishing — this
  topic owns the build-reel product and receipt contract, while a future
  tactical must route implementation through the appropriate shared renderer,
  capture, asset, and platform-adapter owners;
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
capability, licensed human design evidence, and a web-capable engine make
possible.

## The Reframe: A Cross-Ecosystem Publisher And Generator

GrabCraft, PlanetMinecraft, and similar sites are *catalogues* whose moat is a
large pile of human uploads. We should not try to out-library them with an
undifferentiated upload queue. Owning an engine, a semantic source format, and
AI authoring changes the shape of the product:

- **We are a generator that keeps a greatest-hits gallery**, not a fixed
  library. A prompt ("cozy A-frame with a sleeping loft") yields a build the
  visitor can walk into, in the browser, with no download.
- **Human-authored work is the quality foundation, not obsolete inventory.**
  Licensed exemplary structures teach the system what good plans, massing,
  materials, interiors, and detail look like. Human builders also remain
  first-class catalogue creators with attribution and storefronts.
- **One semantic build publishes to several honest destinations.** Web guides,
  Minecraft Java artifacts, Bedrock artifacts, and Mclone worlds are outputs
  of one reviewed source rather than separate catalogues.
- **Walkable is the unfair advantage, not a conversion demand.** A visitor can
  step inside in the browser and still leave with only a Java or Bedrock build.
  Mclone may power the experience quietly and offer a richer continuation
  without making adoption a condition of receiving value.
- **On-demand authoring expands the catalogue** from user requests and
  refinements, while editorial acceptance and licensed human work prevent an
  infinite low-quality feed.
- **Every accepted build can manufacture its own acquisition media.** The
  semantic structure is also the source for a repeatable construction reveal,
  cinematic hero views, thumbnails, and partner-branded social variants. The
  downloadable structure and the video that attracts attention cannot drift
  apart because both compile from the same accepted source.

Everything below serves that inversion.

## Product Pillars (Value)

These follow directly from owning the engine and a deterministic semantic
record. Several are things a screenshot catalogue structurally cannot do.

- **Walk-through / step inside.** Every build page can drop the visitor into
  the real engine for a first-person tour. The page may describe this neutrally
  as an interactive walk-through; "continue in Mclone" is a separate optional
  action.
- **Build-along mode — honestly scoped.** Because the structure is semantic,
  the engine can ghost the next block(s), track progress, and let a player
  follow the build in Mclone. Equivalent generated layers and material deltas
  remain useful to Minecraft users without an in-game integration. This is
  **not** universal: in creative, pasting the whole structure wins and
  build-along is pure friction. Its real homes are **survival** (gathering and
  placing is the game), **survival multiplayer servers** (pasting may be
  banned), and **onboarding**. Offer paste, guided construction, and ordinary
  downloads according to target and intent; do not pretend build-along beats
  paste.
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
- **Build-reel compiler.** Named components, build layers, a deterministic
  camera path, and a reviewed shot grammar turn each accepted build into a
  satisfying short-form construction reveal. This is a product output and
  acquisition surface, not discretionary launch marketing.
- **Survival-friendliness metadata.** Filter by buildable-in-survival, material
  rarity, and estimated time.

## Cross-Ecosystem Publication Contract

The catalogue should be able to serve a visitor who never installs Mclone.
One licensed canonical structure may compile into a deliberately bounded set
of target artifacts:

```text
licensed semantic structure
  +-- interactive web guide and material list
  +-- Minecraft Java structure/schematic or world artifact
  +-- Minecraft Bedrock structure/add-on or world artifact
  +-- Mclone runtime structure and playable-world destination
  +-- deterministic build reel, cutdowns, thumbnails, and hero assets
```

The canonical record stays target-neutral where the concept is genuinely
shared. Semantic roles such as primary timber, foundation stone, roof accent,
entrance, room, and socket resolve through explicit target adapters. Exact
block-state output and unsupported-feature substitutions are target receipts,
not silent reinterpretation.

Useful compatibility classes include:

- **Vanilla-compatible:** constrained to a reviewed Java/Bedrock palette and
  behavior set, with an equivalent Mclone presentation where possible;
- **dual-target semantic:** preserves composition and material roles while
  declaring exact per-target substitutions; and
- **Mclone Enhanced:** may use original Mclone blocks, creatures, functional
  markers, or simulation that cannot honestly export to Minecraft.

The product must label these classes before download. "Works in Bedrock" may
not mean a screenshot-only resemblance or an untested conversion. Each
published artifact needs target version, dimensions, block-state mapping,
unsupported-feature, and validation receipts.

This later output matrix does not make vanilla NBT, Bedrock formats, or public
uploads hidden requirements for Structure Lab's completed first proof. Target
compilers consume the canonical record after it has passed the existing source
and drift gates.

## Growth And Distribution

The platform serves two compatible loops. Minecraft publication may end in a
Java or Bedrock world; Mclone publication may continue directly into the full
web client. Neither path is a decoy for the other.

- **Programmatic SEO is the primary lever.** Every generated build is a landing
  page for a long-tail query, and each page can also be a one-click interactive
  walk-through. Publication capacity is bounded by compute and editorial
  quality rather than only human upload rate.
- **Zero-install "Walk through" on every compatible page**, because the web
  engine already runs. "Open in Mclone," Java download, and Bedrock download
  remain distinct, honest actions.
- **Shareable deep links** into a build, a walk-through, or a "spawn into this
  world" — Discord/Reddit/TikTok-native, each share a demo link into the
  engine.
- **Auto-generated build reels/GIF/OG cards.** Compile a semantic construction
  reveal, hero views, and destination-specific calls to action for every
  accepted build. This repeatable media format is the primary social
  acquisition candidate, not merely a thumbnail convenience.
- **Embeddable "walk this build" iframe** places the engine on other MC
  blogs/wikis — parasitic distribution.
- **Request queue → agents fill it.** People request builds; agents author
  them under review. Requests are product research, engagement, and potential
  fresh landing pages, not an automatic right to publish model output.
- **Cadence and community loops** — build-of-the-day, seasonal drops, "beat
  this base" challenges — agent-supported and editorially reviewed.

## Build-Reel Compiler And Social Acquisition

An accepted structure should produce its own acquisition media as routinely as
it produces a material list or preview mesh. The product is not merely a
downloadable build with occasional promotional videos. It is a build source
whose deterministic publication pipeline can continuously emit polished,
scroll-stopping construction reveals.

A representative Sereyka vertical clip reviewed on 2026-08-20 lasted about
forty seconds. It opened with a footprint/title hook, moved through repeated
cinematic shots in which architectural subassemblies appeared in satisfying
stages, finished the castle through several module reveals, and ended on hero
views plus build-guide and download calls to action. The public clip does not
identify its capture stack. Replay Mod camera paths, Minecraft scripting,
successive world states, shaders, and ordinary video editing are plausible
ingredients, not verified facts about Sereyka's workflow.

The durable product contract is independent of that private toolchain:

```text
accepted canonical structure
  + semantic components and vertical layers
  + target, creator, and storefront identity
  -> deterministic reveal recipe
       + construction phases and stable within-phase ordering
       + camera spline, framing, timing, lighting, and environment
       + title, attribution, destination, and call-to-action slots
  -> renderer or target capture adapter
  -> vertical reel + cutdowns + hero stills + thumbnail/OG assets
  -> media receipt tied to the exact structure and destination hashes
```

The compiler should preserve these principles:

- **Reveal semantic construction, not only Y slices.** A useful default story
  is site/footprint, foundation, structural frame, walls and openings, roof,
  trim, interior, landscaping, and lighting. Named modules such as a porch,
  tower, wing, or gate may become their own beats. Vertical-layer reveal is a
  valid fallback, not the whole visual language.
- **Use a small reviewed shot grammar.** A short needs a first-second hook,
  readable construction beats with camera parallax, one or two moments of
  completion, clean hero coverage, and a concise destination action. A
  repeatable grammar matters more than inventing every clip from scratch.
- **Keep the recipe target-neutral.** The same construction and shot intent
  may drive the baked web catalogue, a first-party Mclone renderer, or a later
  Java/Bedrock capture adapter. Mclone rendering offers scalable automation;
  native Minecraft capture may offer greater authenticity to Minecraft
  audiences. They are complementary outputs when both are supportable.
- **Represent the footage honestly.** A first-party render may be labeled a
  preview render, but it must not be presented as literal Java or Bedrock
  footage. Every visible structure phase must resolve to the same accepted
  build and declared target substitutions; generic AI-video imagery is not a
  trustworthy product preview.
- **Use AI as a director before using it as a pixel generator.** Rules can
  establish the first reveal and camera recipes. Later, an agent can select
  semantic beats, propose paths, vary hooks, and rank deterministic renders
  against measured performance. Reproducible engine pixels keep the advertised
  artifact exact while AI supplies taste and iteration.
- **Ship a complete media bundle.** Normal outputs should include a primary
  9:16 construction reel, shorter cutdowns, hero stills, catalogue thumbnails,
  OG cards, creator/co-brand variants, and manifests for source, render,
  attribution, font, audio, and destination-link provenance.

The smallest proof should use one already accepted structure, its existing
named components and vertical groups, one declarative reveal/camera recipe,
and deterministic vertical capture. It should prove that the result is
visually compelling before adding automatic camera planning, learned shot
selection, a render farm, native-Minecraft capture adapters, or high-volume
publishing.

## Co-Branded Creator Storefronts

A creator with an audience should not have to become a catalogue, file-format,
account, payment, and support operator. The platform can host one underlying
catalogue and service while giving a partner a credible branded entry point:

```text
creator domain or subdomain
  -> creator theme, profile, collections, and attribution
  -> central catalogue, compatibility artifacts, accounts, and checkout
  -> co-branded reels, deep links, and ready-to-publish media bundles
  -> referral commission and/or licensed-content compensation
```

This is closer to a white-label affiliate storefront than a franchise. The
central operator remains visibly responsible for the service, terms, payment,
privacy, moderation, support, and artifact correctness. The creator owns their
voice and curation surface without pretending to be the merchant or runtime
operator when they are not.

The model supports two independent contributions:

- **build and curate:** submit original structures, briefs, revisions, or
  collections under explicit catalogue and optional training licenses; and
- **show and distribute:** publish videos, tutorials, or social posts whose
  attributable paid referrals can earn an ordinary affiliate commission.

Do not collapse those contributions into one opaque "revenue share."
Catalogue licensing, training-data rights, affiliate attribution, usage
credits, and any cash royalty have different evidence and contracts. Early
cash compensation should prefer attributable referrals and deliberately
licensed featured structures over a fraud-prone per-view pool.

### Observed Sereyka proof

Sereyka is a useful current proof of this product assembly, not a template to
copy uncritically:

- the central service sells interactive layer guides, material lists,
  Litematic/world downloads, and subscription access;
- its Java mod and Bedrock add-ons advertise catalogue browsing, hologram
  preview, placement, build-along layers, and automated construction;
- its short-form content uses a repeatable footprint, staged construction
  reveal, cinematic hero-view, and catalogue-call-to-action format; the exact
  Minecraft capture, camera, shader, and editing toolchain is not public;
- its About page self-reports more than 700 million views, 31 million likes,
  and 1.3 million followers across its content and channels; these remain
  first-party marketing claims rather than independent audience evidence;
- its creator page invites both build submissions and promotional videos while
  keeping income and payout details private; and
- its terms describe affiliate commissions and partner domains as marketing
  storefronts while Sereyka remains the seller, account owner, and payment
  processor. `rafaelabuilds.com` is a live co-branded instance of that shared
  platform shape.

Public follower, view, review, subscriber, or revenue numbers are marketing or
third-party evidence until independently verified. The durable lesson is the
system shape: social content, useful catalogue, cross-edition fulfillment,
creator attribution, and one centralized service.

## The Prompt-Iteration Economy

The credit/token is metered against the thing with the clearest real marginal
cost — **AI authoring inference** — while discovery, play, and a meaningful
compatibility tier stay free. A broader catalogue subscription or paid
compatibility-artifact offer remains a separate open commercial decision.

The iteration loop is the clearest AI-native paid product: "the tower looks
weird," "fix the door," "make the windows bigger." That conversational
refinement is genuinely expensive (each step is an agent pass plus a re-bake)
and genuinely valuable, so it is an honest place to put the first meter.

### Why inference is the cleanest first meter

- **Discovery and the demo stay frictionless.** Browsing, walking in, playing,
  and at least one useful guide/export tier are cheap to serve (static meshes,
  deterministic files, and the engine) and free to the user, protecting both
  the standalone service and the optional Mclone path.
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
- **Human construction remains a first-class creative act.** A builder who
  authors the plan, structure, detailing, or accepted revisions receives
  explicit credit distinct from a director who prompts or curates a generated
  result. One build may name both roles without flattening them into "creator."
- **Distribution can pay separately.** A builder or video creator may also
  earn an attributable affiliate commission through a co-branded storefront;
  that cash path does not require pretending every view measures authorship.

### Real money, soft currency, and guards

- **The leading customer charge is compute:** buying bespoke generation and
  iteration, with a free taste tier and a usage-funded path so the passionate
  and the popular rarely pay cash. Catalogue subscriptions, paid exports, or
  commercial licenses require separate value and current platform-policy
  justification rather than being assumed.
- **Prefer soft currency + reputation early.** Earned credits and rep spent on
  cosmetics, palette unlocks, featured placement, or generation quota avoid the
  fraud/legal/moderation swamp of real-money-per-use.
- **License exceptional human work explicitly.** Commissioned structures,
  catalogue rights, source-file rights, training rights, and derivative-use
  rights are separate grants. Training participation must be opt-in rather
  than inferred from public visibility or a normal catalogue submission.
- **Two failure modes that must be designed for:**
  - *Credit anxiety kills exploration.* Show cost before the action and keep
    refinements cheap and predictable, or people watch the counter instead of
    playing with ideas.
  - *Earn→compute is a direct cost leak.* Because credits buy inference (a real
    cost), "earn when used" is farmable by sock-puppet usage. The *earn* side
    needs per-real-user dedup/rate-limiting (or earned credits discounted vs.
    bought); the *spend* side is safe.

## AI Quality System And Licensed Human Corpus

The valuable AI asset is not a prompt that says "make a good Minecraft
house." It is a rights-clean corpus, an authoring workflow, and a review loop
that make taste and correctness measurable. Build those layers in this order:

1. **Authoring skill.** Teach a capable general agent the Structure Lab DSL,
   house rules, architectural vocabulary, target compatibility contracts,
   compiler commands, render views, and revision procedure. A skill improves
   repeatability and tool use; it does not change model weights or create
   visual taste by itself.
2. **Licensed retrieval library.** Retrieve a small set of relevant examples
   by building type, style, floor plan, roof form, palette, scale, and target
   instead of dumping the whole corpus into every request.
3. **Deterministic and multimodal evaluation.** Reject invalid topology,
   inaccessible interiors, impossible stairs, unsupported target blocks,
   exposed backs, monotonous facades, incoherent roofs, implausible scale,
   excessive survival cost, and close copying. Pair measurable structure facts
   with rendered multi-angle critique and bounded human acceptance.
4. **Fine-tuning or preference optimization, later.** Train only after the
   corpus and evals can distinguish improvement from stylish failure. Model
   customization is an optimization over a proven production loop, not the
   first step.

The preferred gold example contains more than screenshots:

- original schematic, world region, or canonical structure source;
- exterior and interior renders from standardized cameras and lighting;
- the creator's brief and semantic room/component/material annotations;
- target-specific palettes and known substitutions;
- human critique explaining why the design succeeds or fails;
- revision history showing how defects were corrected; and
- explicit catalogue, retrieval, training, and derivative-use rights.

Screenshots can teach presentation and surface taste, but not hidden interiors,
block topology, construction order, or intent. Structure source plus critique
is the higher-value evidence. Commissioned first-party examples,
rights-explicit community submissions, and permissively licensed sources are
preferred over scraping public or paid catalogues.

Generation should remain hierarchical rather than asking a model to predict a
large unordered block cloud:

```text
brief
  -> program, site assumptions, and compatibility class
  -> floor plan, massing, circulation, and component graph
  -> semantic materials and detail vocabulary
  -> typed Structure Lab source
  -> deterministic validation and compilation
  -> multi-angle render critique
  -> bounded revision and human acceptance
```

The system also needs a similarity audit against its licensed reference corpus.
Do not claim a defensible per-output percentage of "influence" from opaque
model behavior. If a retrieved build is directly adapted, record and display
that provenance; otherwise compensate training-corpus participation through
the explicit license or a clearly defined pool rather than fictional
derivative accounting.

## Strategic Thesis (Moat)

The moat is the *integration*, not any single piece: a licensed creator
network, a rights-clean human quality corpus, an AI pipeline that authors and
revises on demand, a deterministic semantic format, a build-reel compiler,
cross-target compilers, and an in-browser engine that can walk or play the
result. A competitor can bolt an LLM onto "generate a schematic," but
reproducing reviewed semantic source + acquisition media + Java/Bedrock/Mclone
publication + correct guides + optional playable continuation is the hard
part.

The compounding advantage is **quality and usage evidence ordinary screenshot
catalogues cannot get**: not only views or downloads, but whether people walked
through, remixed, placed, followed, or completed a build, which target they
used, which substitutions failed, and which human critiques caused accepted
improvements. Use that evidence to rank, retrieve, and commission future work;
do not silently convert private play into training data.

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
- **Minecraft users are not failed Mclone conversions.** Java and Bedrock
  guides and artifacts may be the visitor's intended endpoint. Mclone's richer
  walk-through, world, remix, and simulation features should win interest by
  being useful, not through download friction, missing files, interstitials,
  or degraded compatibility output.
- **Compatibility is an adapter, not product-direction leakage.** Minecraft
  output does not make Mojang assets, Minecraft world generation, vanilla
  parity, or Minecraft block identity authoritative for Mclone. Structures and
  examples must be original or explicitly licensed; public artifacts may not
  redistribute extracted reference assets.
- **Acquisition media must remain truthful.** The reel, guide, preview,
  download, and playable destination must identify the same structure revision
  and target mapping. Do not use generative-video spectacle that advertises a
  building the customer cannot obtain, and do not label first-party Mclone
  renders as native Minecraft capture.
- **Media provenance matters too.** Fonts, music, sound effects, shaders,
  textures, creator identity, target branding, and footage all need rights and
  attribution receipts appropriate to commercial short-form distribution.
- **Commercial Minecraft integration needs current policy review.** The
  Minecraft usage guidelines, EULA, trademark rules, mod constraints, Bedrock
  creator terms, Marketplace path, and target technical limits must be checked
  against the exact offer before implementation or launch. In particular, do
  not assume that a mod may authenticate an out-of-game paid entitlement to
  unlock in-game functionality merely because another service currently does
  it.
- **Co-branding must preserve accountability.** A partner domain may express a
  creator's identity and collections, but the merchant, account operator,
  privacy controller, support boundary, disclosures, and payout basis must be
  explicit.
- **Public visibility is not training consent.** Catalogue, retrieval,
  fine-tuning, derivative, and promotional rights are separate. Store the
  rights receipt with the source and exclude expired or withdrawn material
  according to the governing agreement.
- **The economy must never gate the top of the funnel.** Metering lives on
  bespoke inference only. Discovery, preview, and honest target compatibility
  should not become deliberately bad so a paid or Mclone-only path looks
  better.
- **The economy serves acquisition/retention, not vice versa.** Do not let the
  token layer become the product.

## Open Questions

- What is the smallest flagship worth building first: cross-target publication
  for one excellent build, walk-through + build-along, the programmatic-SEO
  page engine, the build-reel compiler, or prompt-to-playable-world?
- Can the existing baked GLB groups produce the first professional vertical
  construction reveal, or does the quality bar require the shared Mclone
  renderer from the first proof?
- What minimal component/phase metadata lets a reveal read architecturally
  without turning video direction into a second structure-authoring surface?
- Which target-neutral camera and reveal recipe can drive both first-party
  rendering and later Java/Bedrock capture adapters without promising visual
  identity those targets cannot reproduce?
- Which reel completion, replay, save/share, click-through, catalogue use,
  artifact download, and Mclone-open measures distinguish entertaining views
  from useful acquisition?
- Which initial Java and Bedrock artifacts are both technically supportable and
  commercially acceptable, and which target/version receipts prove them?
- What semantic palette and compatibility classes preserve a coherent design
  across Java, Bedrock, and Mclone without making lowest-common-denominator
  content the default?
- What naming and disclosure make a creator-branded domain feel genuinely
  theirs while keeping the central merchant and service boundary clear?
- What are the concrete free-tier size, refinement vs. generation prices, and
  earn rates, and where (if anywhere) does real money first appear?
- What sybil/dedup rule makes "earn credits when used" safe given earned credits
  buy real compute?
- How fast must the iterate→re-bake loop feel before conversational refinement
  is delightful rather than a chore, and what latency budget does that imply?
- Should earned and purchased credits be fungible, or should earned credits be
  discounted/limited to a subset of sinks?
- Which corpus license, withdrawal policy, provenance record, and compensation
  model are fair to human builders without making unprovable influence claims?
- Which human ratings and deterministic/multimodal evals are predictive enough
  to justify fine-tuning, and how large and diverse must the accepted corpus be
  first?
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
- [`voxel-sandbox-competitive-landscape.md`](voxel-sandbox-competitive-landscape.md)
- [`distribution-go-to-market.md`](distribution-go-to-market.md)
- [`../native-web.md`](../native-web.md)

## Dated External Evidence

Checked 2026-08-20:

- [Sereyka Builds](https://sereyka.com/) — central catalogue, price, guides,
  downloads, and public product shape.
- [Sereyka About](https://sereyka.com/about) — first-party description of the
  layer-guide product and self-reported cross-channel audience totals.
- [Representative Sereyka construction reveal](https://www.tiktok.com/@sereykamc/video/7642036261586291990)
  — reviewed shot structure and call-to-action sequence; not evidence of the
  private capture or editing toolchain.
- [Sereyka mod and Bedrock add-on page](https://sereyka.com/mod) — advertised
  Java/Bedrock installation, hologram, placement, layer, and auto-builder
  paths.
- [Sereyka creator page](https://sereyka.com/creators) — build-submission and
  video-promotion participation paths; payout details remain private.
- [Sereyka terms](https://sereyka.com/terms) — affiliate commissions,
  partner-domain boundary, centralized seller/account/payment posture, and
  content-license terms.
- [Rafaela Builds](https://rafaelabuilds.com/) — live co-branded partner-domain
  example backed by the shared Sereyka platform.
- [Minecraft Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines)
  — dated brand, website, commercial, video, and mod constraints; not a
  substitute for legal review.
- [Minecraft Bedrock structure documentation](https://learn.microsoft.com/en-us/minecraft/creator/documents/structures/introductiontostructureblocks?view=minecraft-bedrock-stable)
  and [structure API](https://learn.microsoft.com/en-us/minecraft/creator/scriptapi/minecraft/server/structure?view=minecraft-bedrock-stable)
  — official `.mcstructure`, import, placement, and API evidence.
- [Minecraft Partner Program](https://www.minecraft.net/en-us/partner) —
  official Bedrock Marketplace publication route.
- [Replay Mod documentation](https://www.replaymod.com/docs/) — Java-side
  precedent for smooth keyframed camera paths and direct video rendering; not
  evidence that Sereyka uses it.
- [Minecraft Bedrock Editor Camera Tool](https://learn.microsoft.com/en-us/minecraft/creator/documents/bedrockeditor/editorcameratool?view=minecraft-bedrock-stable)
  and [Free Camera Script API tutorial](https://learn.microsoft.com/en-us/minecraft/creator/documents/camerasystem/freecamerascriptapitutorial?view=minecraft-bedrock-stable)
  — official spline-camera and scripted-flyover capabilities; not evidence of
  Sereyka's implementation.
- [OpenAI Skills API](https://developers.openai.com/api/reference/go/resources/skills),
  [fine-tuning API](https://developers.openai.com/api/reference/resources/fine_tuning),
  and [evals API](https://developers.openai.com/api/reference/resources/evals)
  — current separation between reusable skill bundles, training, and
  evaluation facilities.
