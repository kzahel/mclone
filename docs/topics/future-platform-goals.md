# Future Platform Goals

Topic: `future-platform-goals`

Status: planning record created 2026-07-24. The platforms below are candidates,
not supported targets, release promises, or authorization to begin app
scaffolding. Current product targets and their validation lanes remain owned by
[`../platforms.md`](../platforms.md).

## Scope

This topic records:

- the eventual client-platform portfolio Mclone should remain able to pursue;
- the public access, development-hardware, and publishing path for iOS,
  Nintendo, Xbox, and PlayStation;
- how the current shared Rust engine helps or obstructs those ports;
- the architectural work that is worth doing before proprietary SDK access;
- a cost and sequencing model; and
- the evidence required before promoting a candidate to a supported target.

It does not contain confidential SDK facts, substitute public homebrew paths
for licensed console development, or make closed-platform code part of the
public shared-engine contract.

Public program facts on this page are a dated snapshot. Console program terms,
hardware availability, fees, target generations, certification rules, and
store policies must be rechecked with the platform holder before spending or
announcing support.

## Direction

Mclone is well positioned at the gameplay and host-architecture level, but it
is not yet one adapter crate away from a commercial console build.

The shared scene, runtime, renderer-facing view data, input semantics,
persistence port, asset system, integrated/dedicated session model, and
offscreen validation lanes are the right foundation. The main uncertainty is
below those contracts:

1. commercial console Rust toolchains are not upstream, ordinary supported
   Rust targets in the way iOS is;
2. public `wgpu` supports Vulkan, Metal, D3D12, GLES, and browser WebGPU, but
   does not advertise Nintendo, PlayStation, or Xbox-console backends; and
3. parts of the current Cargo graph still treat every non-WASM target as a
   desktop-like native host with SQLite, CPAL, native sockets, threads, and a
   normal filesystem.

The desired long-term portfolio is:

| Class | Direction |
|---|---|
| Windows, macOS, Linux, SteamOS / Steam Deck | Current first-class desktop-flat family. Windows handhelds consume this family; they are device-acceptance lanes, not new engine ports. |
| Desktop OpenXR | Current first-class desktop-XR family, including SteamVR-compatible runtime paths where supported. |
| Flat Android | Current first-class mobile target. |
| Android XR / Quest | Current first-class standalone-XR target. |
| Web/WASM | Current first-class browser target, not a reduced preview. |
| iOS / iPadOS | Recommended next public-toolchain native target when another product target is justified. |
| Xbox Series X\|S | Intended future console target and likely lowest-risk first closed-console feasibility study. |
| PlayStation 5 | Intended future console target after the renderer/toolchain path is proven. |
| Nintendo's current generation, presently Switch 2 | Desired future handheld/home-console target and especially strong product fit. Public access is currently the blocker. |
| Nintendo Switch | Conditional late-generation target or compatibility bridge, not a default promise. |

Near-term product work should still prioritize shared feature quality and
release readiness over another app shell. "Future console ready" means removing
accidental host assumptions as shared code is touched, not creating speculative
console crates without an SDK.

## What Console Development Actually Requires

A commercial console port is an approval, engineering, and certification
project:

1. Establish the legal publisher/developer account and apply to the platform
   program.
2. Sign the applicable NDA and publishing/developer agreements, then obtain
   project or concept approval where required.
3. Receive the proprietary SDK, documentation, signing credentials, support,
   and development/test hardware.
4. Bring up a platform executable or host library using the vendor build,
   deployment, debugging, graphics, audio, input, storage, and network tools.
5. Integrate platform users, controller assignment, entitlements, privileges,
   achievements/trophies, system UI, virtual keyboard, invitations, presence,
   suspend/resume, save data, and error handling.
6. Hit platform memory, frame-time, package-size, loading, thermal, offline,
   safe-area, and lifecycle requirements on hardware.
7. Package and sign a store build, prepare age ratings and store materials, and
   pass the platform's technical certification/review.
8. Run release-candidate, retail-environment, patch, rollback, crash-reporting,
   and post-launch support lanes.

The development kit is only the device at step 3. It does not replace the
program agreement, SDK, platform integration, QA, or certification.

## Public Access And Hardware Snapshot

As of 2026-07-24:

| Platform | Public entry path | Development hardware | Can an ordinary retail device replace it? |
|---|---|---|---|
| Nintendo Switch | Register on the Nintendo Developer Portal, accept its NDA/terms, then submit a separate Switch access request with project and experience information. Registration and tools are free; hardware pricing is disclosed inside the portal. | Nintendo development hardware must be acquired under the program. Public exact pricing should not be guessed from old reports. | No for normal licensed iteration. A retail unit remains useful later for retail/backward-compatibility acceptance. |
| Nintendo Switch 2 | Nintendo's public portal currently says it is not accepting requests for Switch 2 development-environment access. | Not publicly obtainable through the general application path at this date. | No. Buying a retail Switch 2 does not confer SDK or unsigned-build access. |
| Xbox Series X\|S | Apply through ID@Xbox. Microsoft publicly says there are no application, certification, publishing, or update fees, though a modest one-time Partner Center cost may apply. | The current onboarding page says two complimentary kits are provided after concept approval. The public GDK and PC path allow useful work before hardware arrives. | Not for a shippable GDK console game. Retail Xbox Developer Mode is an app/UWP lane, and Microsoft now says UWP games are no longer accepted in the Xbox Store. |
| PlayStation 5 | Register at PlayStation Partners with a short project plan. Approval plus the Global Developer and Publisher Agreement unlocks tools, documentation, publishing resources, and support. | SIE's public 2022 offer says an accepted new partner may request one complimentary development kit and one complimentary test kit, returnable within two years or earlier on request. Confirm that offer and shipping terms when accepted. | No for ordinary unsigned commercial-game iteration. A retail console is a later retail-build QA device. |
| iOS / iPadOS | Xcode and a free Apple Account are enough for limited personal-device testing. App Store distribution requires Apple Developer Program membership. | An ordinary iPhone or iPad is the development and test device; Xcode requires a Mac. | Yes. This is not a console-style dev-kit program. |

Apple currently prices Developer Program membership at USD 99 per membership
year. Free Personal Team provisioning is limited and expires after seven days,
so it is appropriate for experiments, not a release pipeline.

Primary public references:

- [Nintendo registration](https://developer.nintendo.com/register),
  [process](https://developer.nintendo.com/the-process), and
  [cost FAQ](https://developer.nintendo.com/faq)
- [Nintendo Switch 2 access notice](https://developer.nintendo.com/home/developing-for-switch2)
- [ID@Xbox onboarding and costs](https://developer.microsoft.com/en-us/games/publish)
  and
  [current hardware offer](https://www.xbox.com/en-CA/games/publish/id/welcome)
- [Xbox GDK tools and dev-kit workflow](https://learn.microsoft.com/en-us/gaming/gdk/docs/gdk-dev/get-started/overviews/sdk-and-tools)
  and
  [retired UWP-game publishing lane](https://learn.microsoft.com/en-us/windows/uwp/gaming/getting-started)
- [2026 PlayStation partner path](https://sonyinteractive.com/en/news/blog/showing-your-game-to-playstation/)
  and
  [complimentary PS5 hardware offer](https://sonyinteractive.com/en/news/blog/complimentary-development-hardware/)
- [Apple membership comparison](https://developer.apple.com/support/compare-memberships/)

## Switch 1 Versus Switch 2

The long-term product goal should be Nintendo's current generation, presently
Switch 2. Switch 1 should remain a conditional opportunity rather than be
discarded or promised now.

There is still a real commercial argument for Switch 1:

- Nintendo reports 155.92 million lifetime Switch units and 19.86 million
  Switch 2 units through 2026-03-31.
- Switch 2 can run many compatible physical and digital Switch games, so a
  good Switch release can potentially retain value on Switch 2.
- A portable, controller-first voxel sandbox is a strong fit for the product.
- Switch 1 would impose a useful low-end CPU, memory, storage, and GPU
  discipline.

There are equally strong reasons not to commit:

- new Switch hardware sales are now declining while Switch 2 is growing;
- Switch 1 is the hardest performance floor in the proposed portfolio;
- backward compatibility is per-title, not a promise that every Switch build
  will be fully compatible;
- the current public portal does not provide a Switch 2 development route, so
  the real cross-generation publishing options cannot be evaluated publicly;
  and
- the renderer/toolchain work may dominate any benefit from the install base.

The decision gate after Nintendo approval is:

1. Ask Nintendo which Switch or Switch 2 route is available and commercially
   appropriate for the intended release window.
2. Prove a representative world at a stable, agreed frame-rate and memory
   budget on the weakest supported device.
3. Prove suspend/resume, save quotas, loading, controller reassignment, offline
   play, package size, and update behavior.
4. Test the exact Switch build on Switch 2 through Nintendo's compatibility
   process.
5. Compare the incremental QA/support cost against the addressable audience
   and launch timing.

Until those gates pass, describe the goal as "Nintendo platform support,"
with Switch 2 desired and Switch 1 under evaluation.

References:

- [Nintendo hardware and software unit sales](https://www.nintendo.co.jp/ir/en/finance/hard_soft/index.html)
- [Nintendo's Switch-to-Switch-2 compatibility description](https://www.nintendo.com/us/gaming-systems/switch-2/transfer-guide/)

## Current Mclone Position

### Strong foundations

- `mclone-scene` already owns host-neutral gameplay, session, UI, camera,
  render admission, and mono/stereo orchestration. A console app should be
  another cadence/input/surface/services rim, not another game client.
- Explicit renderer view and target facts keep `winit`, Android activity,
  browser, and OpenXR objects outside the shared gameplay boundary.
- The semantic input stack already separates raw collectors from bindings,
  contexts, controller-layout families, prompts, haptics, and player actions.
  Nintendo-, PlayStation-, and Xbox-like label families already have a neutral
  home even though platform glyph assets and collectors remain future work.
- The completion-based persistence port can accept another record executor.
  Console save-data mechanics do not need to rewrite authoritative save
  semantics.
- Integrated and remote dedicated sessions share protocol and client state.
  Platform networking and identity can be adapters rather than gameplay forks.
- First-party asset packs, transactional replacement, offscreen capture, and
  deterministic smoke hosts reduce the amount of platform-only debugging.
- Quest and Steam Deck force mobile/handheld performance and controller UX
  discipline before a Nintendo port exists.
- Store-owned update behavior is already the release architecture for Quest
  and Steam and extends naturally to console stores.

### Major blockers and risks

#### Graphics

`mclone-render`, `mclone-scene`, and related render contracts use `wgpu`
directly. Public `wgpu` currently lists native Vulkan, Metal, D3D12, and
OpenGL/GLES plus browser WebGPU/WebGL; it does not list commercial console
backends.

The first approved-console spike must choose one of:

1. a vendor- or porting-partner-supported `wgpu` console path;
2. a private console backend maintained under `wgpu-hal` or a comparable
   low-level bridge; or
3. a separate console renderer behind a narrower Mclone render-device
   boundary.

The first option is preferred. The second preserves more renderer reuse but is
a substantial ongoing backend and shader-toolchain commitment. The third is a
last resort because it creates the largest parity surface.

Public `wgpu` now has an additional, encouraging extension seam. Version 30
exposes a feature-gated `wgpu::custom` interface and
`Instance::from_custom`, with external implementations for adapters, devices,
queues, surfaces, resources, and command recording. Work merged in June 2026
expanded the capabilities available to those implementations. This is a
high-level external `wgpu` backend interface, not proof of a console
`wgpu-hal::Api` or a commercially supported console port. It nevertheless
allows an NDA-bound implementation to remain out of tree and supply ordinary
`wgpu` objects to an application without adding public console code or a
public `Backend` variant.

This means the preferred first option may take either of these forms:

- a private console `wgpu-hal` backend integrated with `wgpu-core`; or
- a private implementation of the external `wgpu::custom` interfaces,
  potentially wrapping another vendor or C-level graphics implementation.

Which form preserves validation, shader translation, and upstream
compatibility most effectively must be evaluated against a real candidate.
The presence of the extension point raises the plausibility of private
middleware; it does not demonstrate that such middleware currently exists.

Mclone currently pins `wgpu` 25 and carries a patched `wgpu-hal` 25.0.2. The
current version-30 extension surface must therefore not be assumed to drop
into this workspace unchanged; adopting a candidate may require a deliberate
upgrade or a narrowly maintained backport. The existing HAL patch demonstrates
that controlled HAL work is possible, but it also means a private console
backend would have to track both upstream `wgpu` changes and the proprietary
SDK. A time-boxed triangle, texture, shader, buffer, render-target, and
readback proof should precede a full port.

Public references:

- [wgpu supported platforms](https://github.com/gfx-rs/wgpu#supported-platforms)
- [wgpu custom-backend interfaces](https://wgpu.rs/doc/wgpu/custom/index.html)
- [`Instance::from_custom`](https://wgpu.rs/doc/wgpu/api/instance/struct.Instance.html#method.from_custom)
- [June 2026 custom-backend improvements](https://github.com/gfx-rs/wgpu/pull/9605)

#### Rust target and standard library

iOS is the easy case: upstream Rust distributes full-standard-library ARM64
iOS and simulator targets, cross-compiled with Xcode.

Current Rust also exposes `aarch64-nintendo-switch-freestanding`, but its
official status is tier 3 with no standard library and an NRO-oriented public
tool path. That is useful ecosystem evidence, not the licensed Nintendo SDK,
commercial publishing authorization, or a production Mclone target. It cannot
substitute for Nintendo approval.

There are no equivalent upstream commercial Xbox Series or PS5 targets in the
public Rust target list. An approved port may therefore need a vendor-supported
toolchain, a custom target and `build-std`, a Rust static library linked into a
small vendor-language host, dependency patches, or a porting partner. The exact
choice must follow the confidential SDK rather than public guessing.

References:

- [Rust iOS target support](https://doc.rust-lang.org/stable/rustc/platform-support/apple-ios.html)
- [Rust's freestanding Nintendo Switch target](https://doc.rust-lang.org/rustc/platform-support/aarch64-nintendo-switch-freestanding.html)

#### Capability selection in the Cargo graph

Several shared crates currently use `not(target_arch = "wasm32")` as a proxy
for a conventional desktop/Android native host:

- `mclone-server` selects bundled SQLite through `rusqlite`;
- `mclone-audio` selects CPAL;
- `mclone-net` owns native TCP/UDP sockets and native thread workers; and
- native scene/server runners use standard filesystem, path, thread, and clock
  facilities.

This is acceptable for today's lanes but not a durable console boundary.
Portability should move toward explicit capabilities or backend features:

- platform record executor versus host SQLite;
- platform audio sink versus host CPAL;
- platform transport versus standard native sockets;
- platform job/thread executor versus assumed desktop threads;
- packaged asset source versus host directory discovery; and
- platform surface/device creation versus `winit`.

Do not add a fake `cfg(console)` full of stubs. Add a capability only when a
real consumer exists, keep engine policy above it, and leave desktop, Android,
browser, and console mechanics below it.

#### Save data and storage

The generic persistence record executor is a strong seam, but native catalog
and world-opening code still assumes ordinary paths and SQLite. Console work
must answer:

- which data belongs in platform-managed save storage;
- atomic commit, flush, suspend, sign-out, deletion, and corruption recovery;
- local and cloud quota behavior for unusually large voxel worlds;
- whether large worlds require chunked platform files, a private SQLite port,
  or another record executor;
- how worlds move across devices and platform accounts; and
- what happens when cloud state conflicts or a user loses entitlement.

World saves must not be squeezed into a platform cloud-save product without
measuring its quotas and sync behavior.

#### Platform services and compliance

Each console needs a thin but real service rim for:

- active platform user and controller-to-user assignment;
- sign-in, age/parental restrictions, online privilege, and account loss;
- entitlements, add-ons, achievements/trophies, presence, invitations, and
  activity/session joins where used;
- virtual keyboard, system dialogs, locale, accessibility, safe areas, and
  controller disconnection;
- suspend, resume, power loss, storage removal/full state, and network changes;
- crash reporting and development-only diagnostics;
- package identity, signing, store sandbox, and certification automation; and
- store rules for mods, downloaded code/content, UGC, chat, cross-play,
  accounts, payments, and external links.

These mechanics belong in private platform adapters and neutral shared
service contracts. They do not justify platform-local gameplay.

#### Confidential code and open-source boundaries

If Mclone adopts a public/open core, proprietary SDK headers, libraries,
samples, documentation, generated files, credentials, and NDA-derived
implementation details must stay in an access-controlled build environment.

Prefer:

```text
public or generally shareable mclone workspace
  -> neutral platform-service and render-backend contracts
  -> access-controlled per-console host/backend crates
  -> vendor SDK and signing/package tools
```

Whether the private crates live in an access-controlled sibling repository or
a restricted subtree is an operational decision. The shared API should be
testable with public mock/offscreen implementations so closed code is not
required for ordinary engine development.

## Private Backend And Middleware Discovery

### What the public evidence establishes

As of 2026-07-24, no publicly advertised, commercially supported `wgpu`
backend was found for Nintendo Switch, Switch 2, PlayStation 5, or native Xbox
console development. That is not evidence that no backend exists. A
studio-internal implementation may never be offered to others, while reusable
middleware may be visible only to approved developers through a vendor or
platform-holder portal.

The surrounding engine ecosystem demonstrates both possibilities:

- Godot does not publish official console export templates in its open
  repository. Approved developers instead use their own port or private
  middleware built with the official SDK.
- W4 Games advertises private, source-available Godot console ports to
  platform-certified developers. This proves the practical business model:
  keep the reusable open engine above an access-controlled platform and
  renderer implementation.
- Bevy publicly supports Windows, macOS, Linux, web, iOS, and Android, not the
  commercial consoles. Its `no_std` work makes core engine crates easier to
  bring to restricted targets, but does not supply console rendering, audio,
  storage, lifecycle, or certification support.
- Public Bevy Switch and Xbox experiments are useful feasibility evidence,
  but none located during this review constitutes supported, licensed
  middleware with certified shipping-title evidence.

Public references:

- [Godot console support](https://godotengine.org/consoles/)
- [Godot custom-platform ports](https://docs.godotengine.org/en/stable/engine_details/engine_api/custom_platform_ports.html)
- [W4 private console middleware](https://www.w4games.com/w4consoles)
- [Bevy's public platform list](https://bevy.org/)
- [Bevy console-motivated `no_std` work](https://bevy.org/news/bevy-0-15/#no_std-progress)
- [experimental public Switch/homebrew Bevy adapter](https://github.com/ibrahimcesar/switchbrew_bevy)
- [experimental public Bevy WinRT fork](https://github.com/momo-AUX1/bevy/tree/WinRT)

### Discovery before platform approval

Public investigation should ask for a contact or confidential inquiry path,
not ask an NDA holder to disclose SDK details. Contact:

1. the `wgpu` development and user Matrix rooms and the `wgpu` channel in the
   Rust GameDev Discord;
2. Bevy renderer/platform maintainers and studios known to ship substantial
   Bevy or custom Rust games;
3. console porting houses and middleware vendors that accept custom engines;
   and
4. platform developer-relations contacts, asking whether an approved vendor
   supports Rust, WebGPU, `wgpu`, Naga, or Bevy.

A suitable public inquiry is:

> We maintain a Rust engine using `wgpu` and are evaluating future commercial
> console ports. Are you aware of any licensed, out-of-tree `wgpu`
> implementation or console integration for Xbox GDK, PlayStation 5,
> Nintendo Switch, or Switch 2? We are not requesting confidential details
> publicly; a middleware/vendor contact or process for making an inquiry after
> platform approval would be sufficient.

The official `wgpu` repository directs users to its Matrix rooms and the Rust
GameDev Discord. Public code, issue, job, résumé, and conference searches for
`wgpu`, `WebGPU`, `Naga`, `Rust`, and the platform name can reveal useful
leads, but silence is not a negative result.

Reference:
[wgpu community contacts](https://github.com/gfx-rs/wgpu#need-help-want-to-contribute).

### Discovery after platform approval

The authoritative search begins inside the licensed environment:

1. Search the platform's middleware catalog, documentation, and private forums
   for `wgpu`, `WebGPU`, `Rust`, `Naga`, and `Bevy`.
2. Ask platform technical support whether another licensed developer provides
   compatible middleware or whether the platform holder can make an
   introduction.
3. Ask publicly identified vendors to open a confidential discussion after
   verifying that both parties have the required platform authorization.
4. Ask the private developer community whether any shipped title used a Rust
   or `wgpu` path and whether that implementation is reusable.
5. Obtain comparable technical and commercial proposals before buying a
   custom backend or funding one internally.

Nintendo's public process confirms that approved developers receive platform
SDKs, support, documentation, and private forums. Microsoft's middleware
provider page explicitly requires an authorized Xbox developer profile. These
are examples of why a general web search cannot settle the question.

References:

- [Nintendo developer process and forums](https://developer.nintendo.com/the-process)
- [Xbox tools and middleware providers](https://learn.microsoft.com/en-us/gaming/gdk/docs/tools/tools-and-middleware-providers)

### Evidence levels and vendor due diligence

Treat claims of console support in four levels:

1. a sample or triangle once rendered;
2. one studio has an internal game-specific port;
3. a reusable backend is privately available to other approved developers; and
4. maintained middleware has certified commercial shipping-title evidence.

Only levels 3 and 4 materially reduce Mclone's platform risk. For each
candidate, record:

- exact consoles and hardware generations, including whether Switch 1 and
  Switch 2 are separate products;
- official commercial SDK versus public homebrew or emulator path;
- supported `wgpu` versions, API coverage, limitations, and upgrade cadence;
- whether it plugs into `wgpu-hal`, `wgpu::custom`, or replaces the renderer;
- WGSL/Naga and native shader path, pipeline caching, diagnostics, and shader
  debugging;
- surface presentation, synchronization, memory allocation, readback, and
  device-loss behavior;
- Rust compiler target, `std` coverage, C/C++ host boundary, and supported
  third-party crates;
- source access, redistribution rules, private repository access, and whether
  changes may be upstreamed when they are not confidential;
- integration beyond graphics: lifecycle, input, audio, packaged assets,
  storage, networking, users, and platform services;
- development-kit and continuous-integration support;
- certification responsibility and specific shipped-title evidence;
- support response expectations and named maintainers; and
- one-time, per-platform, per-title, support-contract, and revenue-share
  costs.

Do not accept "we can port custom engines" as evidence that a maintained
`wgpu` backend exists. Conversely, do not reject a credible private solution
merely because its source and documentation cannot be shown before platform
approval.

## Cost Model

There are two different answers to "what does it cost?"

### Access and hardware cash cost

- Nintendo portal registration and tools are free; development hardware cost
  is private inside the portal.
- ID@Xbox publicly charges no application, certification, publishing, or
  update fee, aside from a possible modest one-time Partner Center cost, and
  advertises two complimentary kits after concept approval.
- SIE publicly advertised one complimentary PS5 development kit and one test
  kit as a time-limited loan for accepted new partners; current terms must be
  confirmed.
- Apple distribution is USD 99/year, plus ordinary Mac and iPhone/iPad
  hardware if those are not already available.

This means direct kit fees can be modest. Extra kits, secure lab space,
shipping, replacement, test accounts, retail devices, and parallel-team
capacity can still add cost under private terms.

### Actual delivery cost

Engineering, QA, certification, and lifetime support dominate the budget.
Mclone-specific planning should use broad risk reserves, not an old internet
price for a dev kit:

| Port shape | Provisional planning reserve |
|---|---|
| iOS/iPadOS using upstream Rust, Metal `wgpu`, and existing touch/controller contracts | Roughly 2–6 engineer-months for a release-quality first lane, depending on storage, audio, lifecycle, store, and device QA scope. |
| First console when an approved, maintained Rust and `wgpu` path already exists | Roughly 6–12 engineer-months including platform services, performance, packaging, QA, and certification preparation. |
| First console requiring a custom Rust standard-library/toolchain layer or a new `wgpu` HAL/backend | Treat as a 12–24+ engineer-month engine-port program before normal release support. |
| Additional console after shared console service contracts and a reusable renderer strategy exist | Roughly 4–9 engineer-months, still subject to platform-specific certification and performance work. |

These are internal uncertainty bands, not platform-holder or porting-studio
quotes. Re-estimate after a confidential SDK audit and a hardware feasibility
spike. The budget formula is:

```text
engineering months x fully burdened monthly cost
+ internal and external QA
+ certification/submission iterations
+ additional development and retail hardware
+ ratings, localization, legal/privacy, and store materials
+ post-launch patches and platform-support lifetime
```

An external porting studio may reduce toolchain uncertainty, especially if it
already has a licensed Rust or `wgpu` solution, but a custom engine attracts a
premium and still needs an internal owner. Obtain comparable quotes only after
a representative vertical slice, performance capture, dependency manifest,
save/network specification, and source/build access plan exist.

## Recommended Sequence

### Now: preserve optionality

- Keep new gameplay and UI in shared owners.
- When touching `not(wasm)` dependency gates, replace platform identity with
  demonstrated backend capability where practical.
- Keep renderer views/targets explicit and prevent `winit`, Android, browser,
  OpenXR, or future vendor objects from leaking into `mclone-scene`.
- Maintain controller-complete UI, touch support, safe scaling, suspend-safe
  persistence, offline operation, and constrained performance presets.
- Keep third-party dependency licenses, C/C++ build scripts, dynamic loading,
  filesystem assumptions, and generated shader paths auditable.
- Do not create console packages, SDK placeholders, or public claims yet.

### First public-toolchain expansion: iOS/iPadOS

iOS is the best next proof that Mclone can add a non-desktop native platform
without a platform fork:

- Rust has a supported ARM64 target and full standard library;
- `wgpu` has a Metal backend;
- the project already has macOS/Metal, Android lifecycle, touch, controller,
  mobile-performance, asset-pack, and app-private-storage experience; and
- consumer hardware is sufficient for development.

An iOS proof should still begin in a new thin app target, use the shared Mono
scene host, and inject Apple lifecycle, surface, storage, audio, input, and
packaging mechanics. It must not turn the desktop `winit` app into an iOS app.

### Partner readiness

Apply to console programs when there is a coherent legal studio identity,
original first-party content, a named product, a polished representative
vertical slice, a credible schedule/budget, and a concise platform-specific
pitch. Apply early enough that review, hardware, SDK integration, and
certification do not sit on the release critical path, but not so early that
the concept and performance evidence are vague.

The Nintendo portal can be registered for free before a shipping commitment,
but a Switch access request should describe a credible project. Switch 2
availability must be watched rather than inferred. Xbox's public PC GDK path
can be studied sooner. PlayStation's current application asks for a short
project plan.

### First closed-console feasibility spike

After approval, time-box the first spike before promising a platform:

1. compile a Rust static library with representative dependencies;
2. start the vendor host, initialize logging and crash capture, and survive
   suspend/resume;
3. render a triangle, then one real Mclone frame through the selected graphics
   strategy;
4. run shared simulation/worldgen jobs within a measured memory budget;
5. read packaged assets and persist/reopen a small world through the intended
   save executor;
6. play audio and collect a controller through neutral contracts;
7. connect to a dedicated server in the platform development sandbox; and
8. package, deploy, launch, and capture an automated smoke receipt.

If no maintained graphics/toolchain path emerges, stop and compare a porting
partner, private HAL investment, or deferral. Do not solve the problem by
forking gameplay or silently dropping platform parity.

### Likely ordering

With only public facts available:

1. iOS/iPadOS is the lowest technical-risk new client.
2. Xbox is the most plausible first closed-console engineering study because
   its public GDK is Win32/D3D12-oriented, its PC-first path is open, and
   approved ID@Xbox concepts receive hardware.
3. Nintendo is the strongest new product/platform fit, with Switch 2 the
   desired target, but its public Switch 2 access gate currently prevents a
   responsible technical estimate.
4. PS5 is a valuable intended target after the shared console/toolchain and
   renderer strategy is understood.

Commercial opportunity, platform-holder support, funding, porting-partner
availability, and actual SDK findings may change that order.

## Other Common Indie Targets

Distinguish an engine platform from a storefront or device profile:

| Candidate | Mclone posture |
|---|---|
| Epic Games Store, GOG, itch.io, Microsoft Store, direct desktop | Distribution channels over existing desktop builds. They need packaging, store services, and commercial support, not separate gameplay clients. |
| Windows handhelds such as ROG Ally / Legion Go-class devices | Existing Windows target plus controller, compact-UI, power, resume, and performance acceptance. Steam Deck remains the stronger Linux/SteamOS canary. |
| Pico / Vive standalone Android OpenXR devices | Opportunistic derivatives of Android XR if SDK/store access, OpenXR extensions, controller profiles, performance, and market evidence justify them. |
| PSVR2 | Later derivative after PS5 flat support and a vendor-approved XR path; it is not implied by desktop/Android OpenXR. |
| Apple TV | Technically adjacent after iOS, but a small separate UX/store target; evaluate only with controller-first demand. |
| visionOS | Not an OpenXR drop-in and not a medium-indie default. Defer until the product has a specific spatial-computing case. |
| Cloud streaming services | Usually distribution/certification of an existing PC or console build, not another engine port. Evaluate per service. |
| PS4 and Xbox One | No default target. Accept only if a publisher, platform holder, porting partner, or measured audience makes the cross-generation support cost compelling near the actual release. |

For a medium-sized indie game, the conventional broad portfolio remains PC,
Switch-family, PlayStation, Xbox, and sometimes iOS/Android. Mclone already
covers more unusual surfaces than most through browser, Linux/SteamOS, Quest,
and desktop OpenXR. More simultaneous launch targets are not inherently better;
commercial launches can be staged while shared behavior remains portable.

## Promotion Gates

A candidate becomes a supported target only when all of these are true:

- platform access and publishing rights are confirmed;
- the toolchain and dependency graph build reproducibly in an approved
  environment;
- real rendered Mclone pixels, not only a sample, run on hardware;
- representative gameplay meets recorded CPU, GPU, memory, storage, loading,
  thermal, and battery/power targets;
- local integrated and remote dedicated sessions use the shared behavior;
- controller-complete menus, text entry, disconnect/reassign, safe areas, and
  accessibility paths work;
- save/reopen, full storage, suspend/resume, sign-out, crash/power-loss, and
  update compatibility are tested;
- packaging, signing, sandbox services, retail/test environment, and
  certification prechecks are automated where the agreement permits;
- proprietary code and credentials have an access-control and CI plan;
- ongoing QA hardware, patch support, and store operations have named owners;
  and
- the target is added to `platforms.md` and the platform-parity matrix with
  honest device evidence.

## Open Decisions

- Will the eventual shared/public code boundary permit private console backend
  crates cleanly?
- Is a supported commercial `wgpu` console path available through any platform
  holder or credible porting partner after approval?
- Should the first console investment optimize for technical risk (Xbox) or
  product fit (Nintendo)?
- Does Nintendo recommend Switch, Switch 2, or a cross-generation path for the
  intended launch window once access is available?
- Can a representative Mclone world fit Switch 1 memory, CPU, save, loading,
  and package budgets without unacceptable product reduction?
- Which platform services are launch requirements: achievements/trophies,
  cross-save, cross-play, invites, presence, platform friends, UGC, mods, or
  commerce?
- Are worlds platform-local, first-party-account portable, or platform-cloud
  synchronized, and what quotas make that feasible?
- Is console work owned in-house, co-developed, or outsourced?
- Which platforms launch together, and which are staged after the first public
  release?

## Related Documents

- [`../platforms.md`](../platforms.md) — current supported targets, host
  ownership, and validation matrix.
- [`platform-parity.md`](platform-parity.md) — current behavior and shared
  contract parity across live targets.
- [`platform-boundary-convergence.md`](platform-boundary-convergence.md) and
  [`platform-host-boundary.md`](platform-host-boundary.md) — measured
  shared/platform split and host convergence.
- [`unified-persistence-interface.md`](unified-persistence-interface.md) —
  platform record-executor seam.
- [`controller-input.md`](controller-input.md) — semantic controller,
  assignment, glyph-family, and haptic direction.
- [`performance.md`](performance.md) and
  [`steam-deck-test-bed.md`](steam-deck-test-bed.md) — constrained-device
  performance and handheld evidence.
- [`release-distribution-and-updates.md`](release-distribution-and-updates.md)
  — store-owned packages and update boundaries.
- [`distribution-go-to-market.md`](distribution-go-to-market.md) — staged
  channel strategy and multi-platform launch dilution.
- [`scripting-and-mod-platform.md`](scripting-and-mod-platform.md) — portable
  mods and unresolved store-policy gates.
