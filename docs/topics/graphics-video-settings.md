# Graphics And Video Settings

Topic: `graphics-video-settings`

Status: active product and implementation ledger. The live shared Graphics
page, SteamOS automatic presentation profile, schema-versioned graphics
preferences, and cross-platform Distant Terrain quality presets are
implemented. Selectable output modes, adjustable GUI scale, persistence for
the other live rows, and most renderer-quality controls are not.

## Scope

This topic owns the player-facing graphics/video settings contract:

- the settings which should exist and what they actually control;
- platform-profile defaults versus explicit player choices;
- output, world-render, and UI-render resolution policy;
- shared UI/runtime/host ownership;
- durable graphics preferences and their precedence; and
- the implementation and validation backlog.

It does not own Steam Deck provisioning or deployment; those remain in
[`steam-deck-test-bed.md`](steam-deck-test-bed.md). It also does not turn debug
toggles, server simulation policy, or world persistence into graphics
preferences merely because they currently appear near the same Options hub.

Minecraft Java 1.17.1 remains one comparative settings inventory, while Mclone
owns the settings required by its renderer and product experience, such as
internal world scale. The reference
[`VideoSettingsScreen.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/gui/screens/VideoSettingsScreen.java)
and [`Options.java`](../../reference/minecraft-1.17.1/src/net/minecraft/client/Options.java)
provides historical input for the inventory below rather than a product
baseline.

## Current Product Behavior

The shared Graphics page currently exposes ten rows:

| Setting | Current behavior | Runtime owner | Durable? |
|---|---|---|---|
| Output Resolution | Read-only committed swapchain/output extent; `N/A` when the profile has no conventional flat output | platform host fact projected through `GameFlatPresentationState` | not a preference |
| World Resolution | Read-only internal 3D target extent | flat presentation host plus shared render path | derived, not stored |
| World Scale | Live `Auto`, 50%, 67%, 75%, or 100% selection | shared typed action, desktop `HostEffects`, native flat surface | no; relaunch returns to `Auto` |
| Section Occlusion | Live on/off renderer option | shared settings controller and scene/render host | no |
| Leaf Detail | Live `Blocky` / `Bushy`; changes derived active-pack leaf geometry through a transactional asset epoch | shared catalog/compiler, settings controller, and scene | yes; schema-1 machine-local graphics preference |
| Distant Terrain | Four-stop `Off` / `Low` / `Medium` / `High` selector with explicit Apply/Cancel; selection is side-effect free and presets have identical bounds on all hosts, while an unset choice resolves through the platform profile | shared terrain-view, settings controller, and scene | yes; schema-1 machine-local graphics preference, saved only after the applied target is drawable |
| Fog | Opens the independent Off/Classic/Natural/Ground Haze controls and color/visibility tuning; LOD quality neither enables nor rewrites fog | shared render fog contract, settings controller, and scene | yes; schema-1 machine-local graphics preference |
| Render Distance | Live chunk-distance slider | shared settings controller and scene/runtime | no |
| Frame Pacing | Live VSync / Max FPS / Uncapped cycle when supported | shared action and platform cadence/surface host | no |
| FPS Cap | Live discrete cap cycle when supported | shared action and platform cadence host | no |

The current rows are real controls, not menu placeholders. Capability profiles
may disable a row on a host which cannot implement it. Output and world
resolution deliberately remain disabled facts until real video-mode selection
exists.

The accepted Distant Terrain descriptors are Off with zero levels/slots, Low
with 6 levels and 96 slots, Medium with 8 levels and 128 slots, and High with
10 levels and 160 slots. Low/Medium/High keep four-by-four tiles and render
stride one; proxy vegetation uses 1/2/4 clipmap levels respectively. Native
desktop defaults High; SteamOS, Web, and desktop OpenXR default Medium; flat
Android and Android XR default Low. These defaults apply only while the
stored preference is unset. Explicit stored and launch choices keep the same
meaning on every platform.

The Android-XR default follows matched physical Quest 3 evidence rather than
a platform-specific descriptor. Reversed fog-off RD5 orbit repeats measured
Low at `71.83 / 71.87 FPS` with positive average headroom, Medium at
`70.52 / 70.45 FPS`, and High at `63.98 / 63.78 FPS`. A final rebuilt APK
with no override resolved to Low and submitted `71.86 FPS`; the same binary's
explicit High control submitted `64.07 FPS`. Tactical
[`320`](../tactical/320-cross-platform-lod-quality-presets.md) records the
complete app-work, GPU, draw, residency, pixel, and validation receipts.

The later constant-time frontier lookup in Tactical
[`323`](../tactical/323-quest-frontier-performance-recovery.md) improves the
stationary Low/RD8 preferred path to `13.273 / 14.305ms` app-work p50/p95,
`7.483ms` reported app GPU, and `17.2%` over-period frames. It reaches 72
submissions per second, which is the current accurate shorthand, but the
`14.305ms` p95 remains above the `13.889ms` 72 Hz period. Android XR's unset
default remains Low; the measurement is not a claim of a strict p95 lock.

### Distant Terrain apply boundary

Distant Terrain is an expensive live reconstruction, not an immediate cycle.
Pointer, touch, keyboard/controller focus, and tracked XR rays address any of
the four ordered stops directly. Moving among them changes only shared staged
state. Apply emits one typed target and is disabled while a previous target is
preparing; Cancel returns to the current requested choice without an engine
effect.

The scene distinguishes requested, applied, and persisted values. Enabled
targets persist only after one complete target-ready presentation. A cold
Off-to-enabled request keeps exact-only output while its clipmap and frontier
warm; it never exposes uncertified optional terrain. Rejected construction or
reconfiguration preserves the prior applied and stored values and reports an
inline failure. Off removes the optional representation and can apply and save
immediately. Tactical
[`327`](../tactical/327-atomic-distant-terrain-settings.md) owns this contract
and its regression evidence.

The separate Display page currently owns player model, first-person body, and
crosshair visibility. Those are presentation/gameplay preferences rather than
video-mode settings, and none is durable yet.

### SteamOS profile

The ordinary Linux binary receives `--platform-profile steamos` from the Deck
payload launcher. It is not a separate Deck executable. The profile requests
borderless fullscreen and supplies an automatic internal-world scale:

- the output swapchain and screen-space UI remain at the compositor-provided
  native extent;
- the 3D world remains native through a 1080-pixel-high / 1920x1080 pixel
  budget, then scales proportionally;
- aspect ratio follows the actual output, including 16:10 and ultrawide;
- resize and dock/undock recompute `Auto`; and
- a fixed scale remains fixed for the rest of that process, including across
  resize and dock/undock.

This is the accepted baseline. A stored `Auto` choice must continue to mean
"use the current platform profile and output," not "remember the last computed
number."

### UI resolution and GUI scale are different controls

The current UI is rendered at native output resolution. `GuiScale::from_pixels`
automatically chooses the logical widget scale from that output; there is no
player-facing GUI-scale setting.

Future work must keep these concepts distinct:

- **world render scale** changes the internal 3D target;
- **UI render resolution** chooses whether UI pixels are rendered at native
  output or at a lower presentation target; and
- **GUI scale** changes the logical size of menus, text, and HUD elements.

Native-resolution UI should remain the default. An optional `UI Resolution:
Native / Match World` control is plausible for low-power hardware, but needs
text/readability and cost evidence before implementation. It is not required
for the first production Deck build.

## Menu Growth And Scrolling

Settings-category pages and Controls Help now use the shared clipped scrolling
contract:

- the heading and Back/Done footer remain fixed;
- wheel and trackpad input scroll the content;
- controller navigation scrolls focused rows into view;
- a visual scrollbar reports the current range; and
- compact widths switch the setting grid from two columns to one.

This is sufficient for Deck controller navigation and mouse-emulating
trackpads. Direct touch-drag scrolling and a draggable scrollbar thumb remain
unimplemented. Scroll position is transient UI state and should normally reset
when a panel is reopened rather than becoming a durable preference.

## Persistence Investigation

### Conclusion

Mclone has feature-specific settings-persistence building blocks and a
schema-1 graphics-preference codec. It stores Leaf Detail, Distant Terrain,
and Fog. Other graphics, movement, display, debug, frame-pacing, and audio
choices remain runtime state unless called out below.

The implemented model is deliberately feature-specific: shared Rust owns typed
documents, schemas, defaults, validation, and reconciliation; platform code
owns only filesystem or browser-storage mechanics. Graphics should extend
that model instead of creating a desktop-only config file or placing setting
semantics in a launcher.

### Systems which already persist

| Domain | Shared contract | Native storage | Browser storage | Actual save wiring |
|---|---|---|---|---|
| Graphics preferences | schema-1 `ClientGraphicsPreferences`: Leaf Detail, optional explicit Distant Terrain preset, and Fog | atomic `preferences/graphics-preferences.v1.json`, with an environment path override | `mclone.graphics.preferences.v1` in `localStorage` | loaded by desktop flat/XR, flat Android, Android XR, and web; saved only after the relevant live scene/catalog effect succeeds |
| Input preferences | schema-1 `ClientInputPreferences`: touch sensitivity/mode plus semantic controller preferences | atomic `preferences/input-preferences.v1.json`, with an environment path override | `mclone.input.preferences.v1` plus synchronized legacy touch keys in `localStorage` | loaded by desktop flat/XR, flat Android, Android XR, and web; touch changes are saved by flat Android and web |
| Asset-pack selection | schema-1 logical pack-id set with availability reconciliation | atomic `preferences/asset-packs.v1.json`, with an environment path override | `mclone.assetPacks.v1` in `localStorage` | saved only after a successful asset-epoch apply; restored by interactive native and web hosts which configure pack discovery |
| Local player profile | schema-1 stable local UUID, display name, and creation time | atomic `preferences/player-profile.v1.json`, with an environment path override | `mclone.playerProfile.v1` in `localStorage` | loaded or created at client/session startup; explicit identity reset replaces it |
| Worlds and player/world state | typed world persistence coordinator and world catalog | SQLite/filesystem backends | IndexedDB | durable, but this is authoritative world data rather than client settings |

The input document supports normalization, legacy browser-key migration,
defaulting newly added fields, and rejection of unknown future schemas.
Native input, asset-pack, and profile writes use a temporary file plus rename.
The asset-pack preference also retains temporarily unavailable logical ids and
commits only after the replacement succeeds.

Important limitations:

- desktop and XR hosts load controller preferences, but the current menu does
  not expose the detailed controller fields and desktop has no menu-driven
  input-preference save path;
- flat Android and web save touch changes while preserving the controller
  portion of the same document;
- `AudioSettings` has live engine state but no preference codec or settings
  page;
- `ClientExperienceSettingsState` is a live reducer/projection, not a durable
  preferences object;
- native Factory Reset removes registered graphics, input, and asset-pack
  files before replacing the local profile; and
- web Factory Reset currently removes the asset-pack key and replaces the
  profile, but does **not** remove the versioned input document or its two
  legacy touch keys. That is a known correctness cleanup.

The authoritative world persistence interface should not be reused directly
for machine-local graphics preferences. A small preference document is a
better lifecycle and portability fit than opening a world database before
client presentation can be configured.

### Current storage locations

When no environment override is supplied, native preference paths are derived
from the parent of the configured client-global world root:

```text
APP_ROOT/
  preferences/
    asset-packs.v1.json
    graphics-preferences.v1.json
    input-preferences.v1.json
    player-profile.v1.json
  worlds/
```

Android uses the same native-file codecs under its app-private root. Browser
hosts adapt shared codecs to `localStorage`. Graphics preferences follow the
same app-global, machine-local shape and are not stored inside an individual
world.

## Implemented Foundation And Expansion Direction

`mclone-app-runtime` now owns the feature-specific, schema-versioned
`ClientGraphicsPreferences` contract. Platform adapters provide opaque storage;
they do not choose graphics defaults, validate ranges, or interpret enum
values.

Schema 1 expands only with controls whose live effect and capability contract
are complete. Likely next fields are:

- world render scale mode;
- render distance;
- section occlusion;
- frame-pacing mode and FPS cap where the host supports them; and
- later GUI scale, display mode, and output-mode intent once those controls
  exist.

Debug overlays, fullbright developer behavior, local server cadence, and
temporary benchmark overrides should not silently enter this document.
Movement, controls, audio, accessibility, and gameplay presentation may use
their own focused preference codecs rather than turning graphics preferences
into a catch-all file.

### Implemented XR render-path experiment

Coordinating Tactical
[`280`](../tactical/280-xr-multiview-render-path-workstream.md) and lifecycle
Tactical
[`281`](../tactical/281-cross-platform-dynamic-xr-render-targets.md) implement
an XR-only
`XR Render Path: Dual Per-eye | Array Per-eye | Array Multiview` row. The row
is capability-gated, defaults to dual per-eye, and reports host-confirmed
requested, pending, active, rejected, and recovery state rather than only the
last requested value. Flat desktop, flat Android, and browser profiles omit
the row.

The setting remains intentionally transient. It is an interactive
renderer-regression and performance-comparison control, not yet a schema-1
graphics preference. `--xr-render-mode` may select its initial value and
`--xr-render-mode-cycle` exercises all modes through the same shared UI action,
but neither saves it. Persistence can be reconsidered only after physical
Metal, foveation, and lifecycle-recovery acceptance make a stored experimental
selection safe. The 2026-07-29 Quest RD5 comparison keeps dual per-eye as the
default; retaining all three choices does not imply that multiview is faster.

### Precedence

Startup and live application should use this order:

1. platform capabilities and safety constraints define legal values;
2. the platform profile supplies defaults, such as SteamOS `Auto`;
3. a stored machine-local player choice replaces those defaults;
4. an explicit CLI/test override replaces both for that launch and is not
   saved; and
5. a live accepted menu edit updates runtime state and the stored preference.

Unsupported or out-of-range stored values must normalize or fall back without
making the host unlaunchable. `Auto` remains a stored policy choice whose
effective scale can change with the output. Fixed world scales remain explicit
player choices.

Graphics preferences should be machine-local by default. Syncing a fixed
resolution, display mode, or quality selection between a Deck, desktop, phone,
browser, and XR headset would be actively harmful. A future account service
could sync portable accessibility or gameplay preferences separately.

### Apply and save semantics

Most quality controls may apply live and save immediately after a successful
effect. Distant Terrain uses the staged, target-ready transaction above because
even an otherwise valid preset change may require a bounded full rebuild.
Output modes need a different transaction:

1. enumerate and validate a platform-supported candidate;
2. apply it without overwriting the last-known-good value;
3. show a timed Keep/Revert confirmation;
4. save only after Keep; and
5. automatically restore the prior mode on timeout, focus/lifecycle failure,
   or rejected surface reconfiguration.

Gamescope needs a platform-specific projection of this shared intent. Its
game-resolution envelope is not equivalent to changing a desktop monitor mode.
The UI should present only choices the active host can honor and must preserve
a safe `Auto`/native path across dock and undock.

The graphics file/key is registered with explicit Factory Reset. The web input
keys omitted by the current reset should still be corrected so future
documents are not forgotten one by one.

## Settings Inventory And Remaining Work

Minecraft Java 1.17.1's Video Settings page includes fullscreen resolution,
biome blend, graphics quality, render distance, ambient occlusion, frame cap,
VSync, view bobbing, GUI scale, attack indicator, gamma, clouds, fullscreen,
particles, mipmaps, entity shadows, screen-effect strength, entity distance,
and FOV-effect strength. Mclone currently implements only a subset and adds
world scale and section occlusion. Tactical
[`245`](../tactical/245-retire-chunk-far-lod-runtime.md) removed the
experimental chunk-based distant-terrain rows.

Rows should be added only after their runtime contract exists. The target
inventory is:

| Area | Status / next contract |
|---|---|
| Output facts | output and world extents are live and read-only |
| Internal world scale | live; add persistence and optional later adaptive/dynamic mode |
| Render distance | live; add persistence and ensure server view and renderer limits remain coordinated |
| Section occlusion | live; add persistence |
| VSync, frame pacing, FPS cap | live on supported flat hosts; add typed persistence and finer cap selection |
| Display mode | add Windowed / Borderless / Fullscreen capabilities rather than assuming one desktop model |
| Output resolution and refresh rate | add mode enumeration plus safe apply/revert; preserve Gamescope semantics |
| GUI scale | automatic only; add an Auto plus explicit logical-scale control |
| UI render resolution | native only; keep native default and evaluate optional Match World mode |
| Overall graphics quality preset | absent; do not conflate the focused distant-terrain LOD presets in Tactical [`320`](../tactical/320-cross-platform-lod-quality-presets.md) with a future bundle that also rewrites leaves, grass, world resolution, actors, or effects; manual edits to a later broad bundle should report Custom |
| Distant-terrain LOD quality | preset semantics and platform defaults complete in Tactical [`320`](../tactical/320-cross-platform-lod-quality-presets.md); Tactical [`327`](../tactical/327-atomic-distant-terrain-settings.md) adds a direct four-stop staged selector, Apply/Cancel, target-ready persistence, cold exact-only warming, and recoverable rejection |
| Ambient occlusion | absent as a player control; first define the renderer-quality effect |
| Brightness/gamma | absent; distinguish a player-facing calibrated range from the fullbright debug toggle |
| Clouds | absent system/control |
| Particles | absent broad system/control |
| Mipmaps and texture filtering | absent control; define reload/apply cost and sampler ownership |
| Entity shadows and entity distance | absent controls; require real renderer hooks |
| Biome blend | absent control; requires renderer/mesh policy rather than a cosmetic row |
| Leaf and grass detail | Leaf Detail is live, transactional, persisted, and defaults Blocky; Grass Detail remains absent and should add an independent field/effect before a later overall preset projects both; see [`bushy-leaf-rendering.md`](bushy-leaf-rendering.md) and [`lush-grass-rendering.md`](lush-grass-rendering.md) |
| View bobbing | absent control |
| Screen-effect and FOV-effect strength | absent controls; likely shared accessibility/presentation preferences |
| Camera FOV | absent player control; likely Display or Accessibility rather than a Deck-only graphics row |
| Attack indicator | absent gameplay/UI feature, not merely a video switch |
| Anti-aliasing, anisotropic filtering, shadow quality, and similar modern controls | evaluate only against implemented renderer features and measured value |

### Suggested implementation order

1. Extend the existing graphics preference codec one working field at a time;
   preserve its native/web adapters, accepted-effect save rule, Factory Reset
   registration, and malformed/future-schema coverage.
2. Persist the other already-live controls. Prove relaunch restoration on
   desktop, browser, flat Android where supported, and SteamOS `Auto`/fixed
   behavior.
3. Add GUI Scale because it materially affects controller/touch readability
   and already has a shared automatic calculation.
4. Add host video-mode capability enumeration, Display Mode, output
   resolution/refresh selection, and timed apply/revert.
5. Add renderer settings in dependency order, beginning with controls whose
   effects already have or can gain shared mono/XR paths. A preset comes after
   several independent controls, not before them.
6. Add touch-drag and scrollbar-thumb interaction, then validate every tall
   settings panel with mouse, trackpad, controller, touch, and XR ray input as
   applicable.

## Validation Contract

Persistence acceptance should include:

- typed JSON round-trip, normalization, missing fields, malformed data, and
  unknown-future-schema behavior;
- atomic native reopen and browser reload;
- startup precedence among platform profile, stored value, and explicit
  launch-only override;
- fixed versus `Auto` behavior across resize and dock/undock;
- unsupported stored values on a host with fewer capabilities;
- a failed write which leaves live play usable and exposes diagnostics;
- Factory Reset on native and web; and
- display-mode timeout/revert once output switching exists.

Rendered settings work must continue to capture and inspect the affected
screen. The 2026-07-23 local slice passed the four affected Rust packages and
was visually inspected at 1280x800 and compact 320x140. It still needs a
physical Deck Gaming Mode pass for menu-driven scale changes and
trackpad/controller scrolling.

The 2026-08-21 Distant Terrain transaction slice adds a 12-pair direct
transition matrix plus pointer and focused-navigation coverage. Its native
960x540 and compact mobile-browser Graphics pages and its Low synthetic-stereo
world were inspected under `/tmp`. Wasm, flat Android, and Android XR builds
pass. The staged browser smoke proves no engine/storage change before Apply,
then applies Off -> Low -> Medium -> High -> Low in one process, moves after
the downshift, captures Low pixels, and restores accepted Low after reload.
The physical Quest rerun remains pending because no attached authorized
headset was available at closeout.

## Code Map

- shared settings reducer and capability projection:
  [`client_experience.rs`](../../native/crates/mclone-app-runtime/src/client_experience.rs)
- implemented graphics preference model:
  [`graphics_preferences.rs`](../../native/crates/mclone-app-runtime/src/graphics_preferences.rs)
- existing input preference model:
  [`input_preferences.rs`](../../native/crates/mclone-app-runtime/src/input_preferences.rs)
- existing asset-pack preference model:
  [`asset_pack_preferences.rs`](../../native/crates/mclone-app-runtime/src/asset_pack_preferences.rs)
- existing local profile and Factory Reset:
  [`local_profile.rs`](../../native/crates/mclone-app-runtime/src/local_profile.rs)
- shared settings UI and scrolling:
  [`mclone-ui/src/v2.rs`](../../native/crates/mclone-ui/src/v2.rs)
- shared UI setting/presentation types:
  [`mclone-ui/src/lib.rs`](../../native/crates/mclone-ui/src/lib.rs)
- shared scene effect application:
  [`host_effects.rs`](../../native/crates/mclone-scene/src/host_effects.rs)
- native flat presentation/profile and current session-only state:
  [`app.rs`](../../native/apps/mclone-native-client/src/app.rs)
- native launch profile parsing:
  [`cli.rs`](../../native/apps/mclone-native-client/src/cli.rs)
- browser preference storage mechanics:
  [`web_scene_host.rs`](../../native/apps/mclone-web-client/src/web_scene_host.rs)
