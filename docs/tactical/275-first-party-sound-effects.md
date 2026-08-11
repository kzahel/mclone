# Tactical 275: First-Party Sound Effects

Status: **implementation complete 2026-08-11. Automated content, mixer,
shared-event, web compile, desktop XR compile, flat Android package, and
Android XR package gates pass. Human mix listening remains pending.**

Topic:

- `first-party-sound-effects`

## Instruction Synthesis

Audit the sound effects used by the sibling Tilefun project, establish whether
they can be redistributed, recommend a useful generic baseline for Mclone, and
then implement that recommendation end to end. Mclone currently has an audio
mixer and landing trigger but deliberately suppresses audio in distributable
first-party asset selections. Replace that silence with a curated, legally
redistributable sound bank, shared event semantics, gameplay/UI producers, and
cross-platform preparation and validation. Commit the work in reviewable
milestones.

## Audit Result and Product Decision

Tilefun references 73 OGG files: 55 from Kenney's **Impact Sounds** pack and 18
from Kenney's **RPG Audio** pack. Both packs are published under Creative
Commons Zero 1.0 (CC0). Attribution is not required, but Mclone will retain
source URLs, archive hashes, upstream filenames, and license texts so every
shipped byte remains auditable.

Tilefun's filenames are not a safe semantic catalog. In particular, its
`sand_*` files are Kenney snow footsteps, several fire-like crackles are wood
creaks, and some creature/action effects are repurposed impacts. Mclone must
not inherit those labels. It may audition the same recordings, but its own
catalog must describe their actual source and intended use honestly.

The first distributable bank will contain 119 deliberately selected OGG files:

| Upstream pack | Selected | Purpose |
|---|---:|---|
| Kenney Impact Sounds | 95 | carpet, concrete, grass, snow, and wood footsteps; generic, glass, metal, soft, wood, mining, and plank impacts |
| Kenney RPG Audio | 19 | neutral footsteps, cloth movement, coin pickup, and wood creaks |
| Kenney Interface Sounds | 5 | back, confirm, error, open, and select UI feedback |

This stays within the recommended 80-120 effect baseline while supplying
variant families rather than one repeated sample per action. Source archives:

- <https://kenney.nl/assets/impact-sounds>
- <https://kenney.nl/assets/rpg-audio>
- <https://kenney.nl/assets/interface-sounds>

The larger Nox Essentials Series remains the preferred follow-up for realistic
sand/gravel/mud/stone surface breadth and natural ambience. Its current archive
is roughly 988 MB and contains substantially more material than this tactical
can responsibly mix and review. It will not be imported wholesale or used as a
pretext to call snow sounds sand. Add a separately reviewed subset only after
the baseline has been played in context and the missing surface/ambience facts
are clear.

## Objective

Deliver a complete first-party audio path:

```text
official CC0 archives
  -> reproducible curated import and byte-level provenance
  -> authored first-party pack with audio_content role
  -> shared decoded variant bank and bounded mixer commands
  -> scene-owned local event/cadence/material selection
  -> native, Android, XR, and browser-compatible preparation
  -> strict proprietary-free provenance and automated behavior evidence
```

The Mclone Original selection should produce useful movement, landing,
interaction, pickup, and UI feedback without any local Minecraft assets. A
missing device or browser autoplay restriction must remain graceful and
non-fatal.

## Scope

### Sound families

The versioned first-party catalog should expose semantic families rather than
upstream filenames:

- movement: footstep and landing families for cloth/carpet, stone/concrete,
  grass, snow, wood, and a neutral fallback;
- world interaction: place/break/impact families for stone, glass, metal,
  soft/earth-like, and wood materials;
- player/inventory: cloth movement and item/coin pickup;
- UI: back, confirm, error, open, and select.

Every repeated family must have variants. Selection should avoid immediately
repeating the previous variant, use a small bounded pitch/gain range, and be
deterministic from event identity where replay/test stability matters. It must
not allocate, decode, read assets, or lock on the real-time callback.

### Event producers

Wire effects only to shared semantic events that already have honest product
meaning:

- local footsteps from distance traveled while grounded, with cadence derived
  from motion rather than key-repeat timing;
- local landing using the existing shared landing event and the material below
  the player;
- successful local block break/place actions through the shared client/scene
  outcome path;
- successful pickup/inventory acquisition where a shared event exists;
- shared UI navigation/confirm/back/error/open effects.

Do not put gameplay sound policy in desktop, web, Android, or XR apps. If a
producer does not expose an authoritative semantic outcome yet, add the
smallest shared event contract or defer that producer explicitly instead of
guessing from raw input.

### Material mapping

Use a small engine-neutral acoustic material classification owned with the
audio semantics. Map current block/runtime material facts to that class in one
shared place. Unknown blocks use the neutral family. This tactical does not
attempt exact Minecraft 1.17.1 `SoundType` parity and does not expose Mojang
sound identifiers as the first-party contract.

### Mixing and spatial behavior

Extend playback commands with bounded gain, pitch, and stereo pan. Local UI
feedback stays centered. Local-player body sounds remain lightly centered;
world interaction events pan from listener-relative position in flat clients
and use the same neutral source/listener contract in XR. Full HRTF, occlusion,
reverb, category buses, streaming ambience, and remote entity audio are later
audio tacticals.

Pitch-shifted playback must retain the existing linear resampling and bounded
voice behavior. When a family cannot load, report it once and keep the rest of
the bank usable.

## Asset and Provenance Contract

Checked-in audio lives below `assets/mclone/sounds/kenney/`; versioned catalog
and provenance records live below `assets/mclone/audio/`. Preserve each
upstream base filename inside a pack-specific directory and never overwrite a
different upstream recording under a convenient semantic name.

The repository must include:

- the exact CC0 1.0 license text distributed with each archive;
- official source and asset-page URLs;
- downloaded archive SHA-256 values;
- for every selected file, upstream archive path, repository path, SHA-256,
  byte length, source pack, and semantic family use;
- a deterministic importer/validator that fails on archive drift, duplicate
  destinations, missing selected inputs, hash mismatch, unlicensed sources,
  malformed catalog families, and catalog files absent from provenance;
- no download during an ordinary build, pack build, test, or release bundle.

The canonical archive hashes accepted at planning time are:

| Archive | SHA-256 |
|---|---|
| `kenney_impact-sounds.zip` | `029d734af1582474edf3a694d1b0cebc97c1c152f2f39fa34d4c2bafc5de77f8` |
| `kenney_rpg-audio.zip` | `6dbeaf8544da958d8f2adcb4a4a4b76c1ade34a05f8ab9edccd327da7375f38b` |
| `kenney_interface-sounds.zip` | `f2193d072726d6758a5f7871b2dcc54dcce0d5c35c6f0a62f92549b327c81232` |

The authored pack declares `audio_content`. Generated Fallback Only remains
silent; it must not synthesize or relabel sound. Mclone Original resolves the
catalog and OGG files from the authored first-party pack. Vanilla Reference may
retain its local-only reference inputs, but proprietary-free validation must
prove that Original resolves no Minecraft-reference or unknown source.

## Shared Ownership

- `mclone-audio` owns semantic sound/family keys, catalog validation, variant
  choice, decoding, playback parameters, voice limits, mixer commands, and
  device capability behavior.
- `mclone-assets` owns pack roles, selected-source reads, and byte-level
  provenance reporting.
- `mclone-app-runtime` prepares the selected sound catalog/bank alongside the
  rest of one asset epoch. It does not suppress authored audio merely because
  Minecraft reference content is disabled.
- `mclone-scene` owns listener facts, local cadence, material queries, semantic
  event-to-sound decisions, and frame-boundary replacement of prepared audio.
- `mclone-client`, `mclone-input`, and `mclone-ui` may expose neutral outcome
  events when their existing effects do not distinguish success from raw
  input.
- platform apps own audio-device construction/resume/suspend only. They may not
  choose sound families, variants, gains, pitch, cadence, or materials.
- server simulation remains free of audio-device and sound-bank dependencies.

## Implementation Slices and Commits

### Slice 0: Tactical and source lock

- record the Tilefun audit, bank decision, archive hashes, scope, ownership,
  validation, and deferrals;
- add the commit-series topic and tactical index entry.

Landed as `173f2837` (`Document first-party sound effects plan`).

### Slice 1: Curated CC0 bank and pack integration

- add the reproducible import/validation tool and checked-in selected OGGs;
- add license, provenance, and semantic catalog records;
- give the authored pack the audio role and include all catalog payloads;
- make strict pack validation prove selected first-party audio provenance.

Landed as `a49c18f2` (`Add provenance-locked first-party sound bank`).

### Slice 2: Shared family playback

- load and validate variant families in `mclone-audio`;
- add no-immediate-repeat selection and bounded gain/pitch/pan commands;
- retain callback real-time invariants and graceful partial-bank behavior;
- replace the two hard-coded landing samples with catalog families.

Landed as `2582983a` (`Load first-party sound families in shared audio`). The
two Mojang landing keys remain only as the local Vanilla Reference fallback;
Mclone Original resolves all movement and landing keys through the CC0 bank.

### Slice 3: Shared gameplay and UI producers

- add shared acoustic-material mapping;
- emit footsteps and material-aware landings;
- connect successful break/place, pickup, and UI outcomes where honest shared
  events exist, documenting any producer whose success contract is absent;
- keep platform apps free of sound policy.

Movement and UI landed as `1cfb5532` (`Add shared movement and UI sound
producers`). Replica-confirmed break/place outcomes landed as `72fa5f80`
(`Confirm block interaction sounds from replica state`). A submitted command is
not treated as success: the scene retains a bounded intent and emits only after
the client replica shows the expected block change. Rejected, denied, expired,
unloaded, and actor/tool interactions remain silent.

Pickup is explicitly not connected. The current shared protocol/replica path
has no authoritative local item-acquisition event, so playing from proximity,
raw input, or an optimistic command would invent success. Add pickup sound when
that owner exposes a neutral outcome event.

### Slice 4: Platform and release validation

- exercise first-party asset build/validation and catalog/provenance tests;
- run the affected Rust workspace and focused movement/interaction/UI tests;
- build/smoke desktop, web/WASM, flat Android, and Android XR boundaries;
- record device/browser capability gaps without converting them into engine
  silence or platform-specific behavior;
- update Tactical 091 and the asset-pack topic with the new durable state.

Completed in this closeout commit. Native desktop, desktop XR, flat Android,
and Android XR retain the existing shared CPAL output path. Web prepares and
validates the same sound bank but keeps the explicit unavailable capability;
browser output and autoplay-resume glue remain deferred under Tactical 091.

## Implementation Outcome

- Mclone Original ships 119 CC0 OGG files selected from the three Kenney packs,
  grouped into 33 semantic families. All bytes are checked in beneath a stable
  pack namespace and covered by per-file SHA-256 provenance.
- The importer verifies the three official archive hashes and fails on source,
  license, file, catalog, or destination drift. Ordinary builds never download.
- First-party preparation decodes each unique OGG once. Variant selection,
  bounded gain/pitch/pan, fixed-capacity voices, and the bounded command queue
  keep decoding, filesystem access, allocation, and locking off the callback.
- Shared scene policy now owns grounded distance-based footsteps,
  material-aware landing, material-aware confirmed block break/place feedback,
  and semantic UI open/back/confirm/select/error feedback for mono and XR.
- Generated Fallback Only remains explicitly silent. Vanilla Reference may use
  the two local, unredistributed Mojang landing samples and does not leak them
  into first-party provenance.

## Validation Evidence

Passing gates on 2026-08-11:

- `pnpm assets:sfx:check`: exactly 119 checked-in OGG files verified against
  provenance and catalog records.
- `pnpm assets:pack:first-party:test`: six builder/packaging tests passed.
- two consecutive `pnpm assets:pack:first-party` builds produced identical
  hashes for all three packs, sidecars, and the staged catalog. The authored
  pack hash was
  `e8f5d40e8b2a03172b4b56dee9381ee8c0bd82489da78e05cb16b82389ee1ae6`.
- `pnpm --silent assets:validate:first-party`: `proprietary_free=true`, 119
  audio samples, 33 audio families, 126 resolved first-party entries, 145
  resolved provisional entries, zero resolved Minecraft-reference/unknown
  entries, two deliberate suppressions, and two optional missing colormaps.
- `cargo test --manifest-path native/Cargo.toml -p mclone-audio`: 11 passed,
  including real checked-in OGG decode plus nonzero, hard-panned 48 kHz PCM.
- `cargo test --manifest-path native/Cargo.toml -p mclone-scene --lib`: 167
  passed, including cadence, teleport rejection, material distinction,
  replica-confirmed interaction, and UI classification.
- `pnpm native:movement:smoke`, `pnpm native:web:build`, and
  `pnpm native:xr:check` passed.
- `pnpm native:android:apk` and `pnpm native:android-xr:apk` passed. Both APKs
  contain the 9,021,735-byte authored pack and generated fallback pack.

The repository-wide Rust test run reaches and passes the affected audio and
scene unit suites, then fails the pre-existing static
`composition_phase_order_is_all_actors_between_all_opaque_and_translucent`
source assertion. The asserted `mclone-app-runtime/src/frame_render.rs` was last
changed by `72fd1504`, before this series. `pnpm native:thin-adapters:purity`
also fails on the pre-existing unregistered
`mclone-terrain-vegetation-worker.ts` module, last changed by `8955d2c4`.
Neither sentinel failure intersects the audio changes.

## Acceptance

Automated acceptance requires:

- exactly 119 provenance-covered selected OGGs and three retained CC0 texts;
- deterministic authored pack bytes across two builds;
- zero Minecraft-reference or unknown resolutions for the Original audio bank;
- variant tests that prove no immediate repeat and stable seeded sequences;
- mixer tests for bounded pitch, gain, pan, voice capacity, and missing samples;
- shared tests for step cadence, material mapping, landing selection, and every
  connected interaction/UI outcome;
- successful affected workspace, web, Android, and Android XR build gates;
- generated fallback still prepares cleanly with explicit silence.

Human listening remains a real acceptance gate rather than something automated
waveform tests can claim. Review at least desktop speakers/headphones and one
mobile/XR device for relative loudness, cadence fatigue, pitch range, clipping,
and whether reused impacts read correctly in context. Until that review is
recorded, implementation may be complete but final mix acceptance remains
pending.

## Explicit Deferrals

- Nox realistic sand/gravel/mud/stone subsets and nature ambience;
- exact Java 1.17.1 sound-event and `SoundType` parity;
- ambient loops, music, streaming decode, and biome/weather beds;
- remote players, creatures, doors, tools, combat, damage, and richer inventory
  effects without existing shared outcome events;
- category sliders, ducking, compression, reverberation, occlusion, HRTF, and
  accessibility/haptic coupling;
- original commissioned recordings or a bespoke Mclone sonic identity pass.
