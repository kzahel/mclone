# Scripting and Mod Platform

Topic: `scripting-and-mod-platform`

Status: **accepted product and architecture direction; runtime selection and
store-policy validation remain open** as of 2026-07-24.

## Purpose

Define a coherent extension model for Mclone across desktop, web, flat Android,
standalone Quest, desktop OpenXR, and dedicated servers. This topic owns the
relationship among data packs, sandboxed gameplay scripts, trusted server or
desktop extensions, source-built forks, package discovery, mod profiles, and
the player-facing mod browser.

This topic does not choose the project's code, content, or trademark licenses.
It defines the technical and product shape that can support a fully open
first-party stack without pretending that source availability alone creates a
safe or usable mod ecosystem.

It complements:

- [`asset-pack-profiles.md`](asset-pack-profiles.md), which owns the current
  client-global presentation-pack lifecycle and provenance contract;
- [`voxel-sandbox-competitive-landscape.md`](voxel-sandbox-competitive-landscape.md),
  which owns the comparative evidence for Minecraft, Vintage Story, Hytale,
  Luanti, Factorio, and other creator ecosystems;
- [`distribution-go-to-market.md`](distribution-go-to-market.md), which owns
  the public open-stack proposition and release-channel decisions;
- [`../native-engine-architecture.md`](../native-engine-architecture.md), which
  owns the shared-engine and platform-adapter boundaries; and
- [`multiplayer-networking.md`](multiplayer-networking.md), which owns the
  carrier-neutral client/server session model.

## Direction In One Sentence

Mclone should have one package ecosystem with a portable,
capability-sandboxed gameplay tier, not separate "powerful desktop" and
"restricted mobile" editions; trusted native/server extensions and complete
source forks remain explicit higher-power escape hatches.

JIT availability may improve execution speed. It must not change a package's
meaning, API, or compatibility.

## One Ecosystem, Several Trust Tiers

The extension model has four deliberately distinct tiers:

| Tier | Purpose | Expected execution surface | Normal distribution |
|---|---|---|---|
| Declarative content | Blocks, items, recipes, entities, structures, world-generation configuration, assets, localization, and declarative UI | Every official client and server | In-game browser, server profile, local import, or bundled first-party content |
| Portable sandboxed logic | Event-driven gameplay through a versioned capability API | Every supported host on which policy validation succeeds; server-side when clients do not need to execute it | Curated or federated package registries and server-required profiles |
| Trusted extensions | Unrestricted Rust/native modules, JVM processes, or similar operator-selected integrations | Dedicated servers and explicitly developer-enabled desktop environments | Manual/operator installation outside the ordinary portable-mod promise |
| Source forks | Arbitrary engine, renderer, protocol, client, or server changes | Any platform the fork author builds and distributes for | Independent source builds and separately signed distributions |

The first two tiers are the ordinary player-facing mod product. The latter two
preserve Java-style breadth and native-library access without treating every
downloaded package as trusted executable code.

Trusted extensions and source forks must not masquerade as portable packages.
A server that requires a custom native client must say so before join. An
ordinary portable profile must never acquire a hidden dependency on an
unrestricted desktop-only module.

## Why Mclone Should Not Split Like Java And Bedrock

A Java/Bedrock-style product split would create two content ecosystems,
different multiplayer expectations, duplicated creator documentation, and a
permanent pressure to make mobile, XR, or web modification less capable. It
would also cut against Mclone's central product advantage: one shared world and
game model across all first-class targets.

The useful distinction is not "desktop game versus mobile game." It is:

- portable, reviewable behavior that uses declared engine capabilities;
- trusted operator code with deliberately narrower distribution; and
- a complete fork with its own build and trust boundary.

This keeps one package identity and one gameplay API while allowing execution
engines and trust policies to differ honestly by host.

## Portable Runtime Direction

WebAssembly is the preferred portable binary and capability boundary to
prototype. It is not yet a locked runtime choice.

The target shape is:

- a narrow, versioned Mclone host ABI with explicitly declared imports and
  exports;
- the browser's built-in WebAssembly runtime on web;
- a JIT or ahead-of-time-capable runtime on desktop and dedicated servers;
- an interpreter as the correctness baseline on Android, Quest, and any host
  where executable-memory or store policy makes JIT inappropriate;
- identical observable package behavior across those execution engines;
- per-instance memory ceilings, execution fuel or deadlines, bounded outputs,
  and crash isolation; and
- no filesystem, network, process, native-library, or arbitrary host access
  unless a specific capability grants it.

The
[WebAssembly Component Model](https://component-model.bytecodealliance.org/design/worlds.html)
is useful design evidence because its worlds describe exactly which functions
a component imports and exports. Mclone does not need to adopt the entire
component toolchain in its first slice, but its host ABI should preserve that
explicit capability shape. [wasmi](https://docs.rs/wasmi/latest/wasmi/) is
representative of a lightweight interpreter with fuel metering for constrained
or no-JIT environments. Wasmtime, Wasmer, and WAMR are candidates to measure,
not dependencies selected by this topic.

WebAssembly is the machine contract, not necessarily the creator-facing
language. Most creators should be able to use:

- declarative schemas for ordinary content;
- generated bindings and high-level libraries;
- visual or form-based authoring for common mechanics; and
- one approachable source language that compiles to or runs over the portable
  contract.

Rust can remain an expert authoring option without becoming the minimum price
of entry.

## Lua Alternative And Guardrail

Lua remains the strongest alternative when authoring simplicity and fast
iteration outweigh a language-neutral binary boundary. It should be compared
in a real prototype rather than added alongside WebAssembly by default.

If Lua wins that comparison:

- Mclone defines one exact Lua language and standard-library profile;
- the interpreter is the semantic and conformance baseline everywhere;
- desktop/server LuaJIT is only an optional accelerator;
- FFI, native modules, arbitrary file/process/network APIs, and implementation
  differences stay outside the portable profile; and
- packages cannot branch their gameplay on whether JIT is active.

Using modern Lua semantics on mobile and different LuaJIT semantics on desktop
without a constrained common profile would create the same platform split this
design is intended to avoid. LuaJIT also documents that its JIT is disabled on
iOS, leaving interpreter mode there:
[LuaJIT installation notes](https://luajit.org/install.html).

The first implementation should not support two general-purpose scripting
runtimes. That would double standard libraries, bindings, diagnostics,
debuggers, security review, compatibility testing, and creator documentation
before either ecosystem is mature.

## Java, C#, And Native Code

Java's reflection, class loading, library breadth, and JNI access explain much
of Minecraft Java modding's power. JNI explicitly permits Java code to call
native libraries:
[Java Native Interface specification](https://docs.oracle.com/en/java/javase/26/docs/specs/jni/intro.html).
Those properties are valuable for trusted operators and total conversions, but
they are not a safe portable package boundary for browsers or store-distributed
mobile clients.

A JVM bridge or out-of-process plugin host may therefore be useful later for
dedicated servers. It belongs in the trusted-extension tier and adapts to the
same authoritative server contracts; it is not the canonical mod artifact.
The same applies to unrestricted native Rust plugins.

Unity's IL2CPP does not provide a model for arbitrary downloadable C# mods on
no-JIT platforms. It ahead-of-time compiles assemblies known to the Unity build
into C++ and then native code. Unity separately documents the runtime code
generation limits of AOT platforms:
[IL2CPP overview](https://docs.unity3d.com/Manual/il2cpp-introduction.html) and
[scripting restrictions](https://docs.unity3d.com/Manual/scripting-restrictions.html).

The unrestricted route remains first-class through open source: a developer
who needs a different renderer, native library, protocol, or game loop can
build a custom Rust client or server. That is a fork or separately trusted
distribution, not an ordinary mod-browser install.

## Authority And Multiplayer

Gameplay logic should be server-authoritative by default. A dedicated,
integrated, or browser-worker `RealmServer` executes rules that mutate durable
world state. Clients receive authoritative results, declarative content, and
presentation assets.

This has several consequences:

- remote clients do not need to execute every gameplay script;
- server-only administration, economy, quest, or simulation packages can work
  with otherwise stock clients;
- client scripts are reserved for bounded presentation, input, UI, and
  prediction capabilities rather than authoritative world mutation;
- singleplayer still exercises the same sandbox through its integrated server,
  so every official local-play host needs the portable baseline for portable
  gameplay packages; and
- a server advertises its exact required profile and compatibility class
  before the client joins.

Minecraft Bedrock provides useful evidence for server-side scripts,
manifest-declared dependencies, API versions, and permissions:
[scripting introduction](https://learn.microsoft.com/en-us/minecraft/creator/documents/scripting/introduction?view=minecraft-bedrock-stable)
and
[multiplayer scripts](https://learn.microsoft.com/en-us/minecraft/creator/documents/scripting/multiplayer-scripts?view=minecraft-bedrock-stable).
Mclone can expose a broader portable API without giving a script ambient native
access.

Full deterministic execution across machines is not a blanket promise.
Server authority normally removes that requirement. World generation or other
systems that genuinely require reproducible results should use deterministic
host primitives, defined numeric behavior, explicit random streams, and
versioned algorithms, or distribute server-generated results.

## Host API Shape

The host API should expose semantic, coarse operations rather than engine
internals. A JIT cannot rescue an API that asks scripts to cross the host
boundary once per block or entity every frame.

Preferred facilities include:

- subscribe to typed lifecycle, tick, interaction, inventory, block, entity,
  and world events;
- register data-defined blocks, items, recipes, components, behaviors, and
  systems;
- run bounded region queries and bulk world edits;
- query and update entities through stable handles and typed components;
- schedule delayed or periodic work without busy polling;
- use deterministic host random, geometry, path, and world-generation
  primitives where relevant;
- store namespaced, schema-versioned package state;
- publish declarative client UI and presentation events;
- exchange typed package messages across an explicitly granted server/client
  channel; and
- receive structured errors, budget diagnostics, and deprecation notices.

Portable packages must not depend on Rust layout, internal ECS types, renderer
objects, app crates, raw pointers, or platform APIs. The host contract belongs
in shared crates and remains neutral to winit, browsers, Android activities,
and OpenXR.

## Capabilities And Resource Budgets

Every package declares requested capabilities. Installation, profile
activation, and server join show the material permissions before execution.
Capabilities should be specific enough to reason about, such as:

- read or mutate world blocks;
- define content and recipes;
- observe or control entity categories;
- create namespaced persistent state;
- publish declarative UI;
- play audio or spawn presentation effects;
- receive player input in approved contexts;
- communicate with its authoritative server peer; or
- use an explicitly approved external network endpoint.

Filesystem, arbitrary external network access, subprocesses, dynamic native
libraries, raw sockets, and unrestricted reflection are absent by default.
Some may exist only in the trusted-extension tier.

Runtime admission should enforce:

- memory and table/object limits;
- per-tick and background-work fuel or time budgets;
- bounded event queues, query sizes, message sizes, and persistent state;
- rate limits for world mutation, UI, audio, logs, and network messages;
- cancellation and watchdog behavior;
- quarantine after repeated failure; and
- structured attribution so diagnostics identify the package, version,
  callback, and consumed budget.

Heavy terrain generation, physics, navigation, meshing, rendering, and bulk
simulation stay native engine work invoked through bounded semantic requests.

## Package Contract

The mod platform should reuse the good primitives already proven by asset
packs—stable identity, manifests, roles, provenance, schema compatibility,
content fingerprints, deterministic resolution, transactional application,
and rollback—without reusing the asset pack's lifecycle incorrectly.

A package manifest should eventually describe at least:

- stable package id, display name, version, authors, and description;
- license, source repository, issue/support links, and content provenance;
- engine/API compatibility and package-format version;
- content class: data, gameplay, world generation, server/admin, client
  presentation, asset, library, tool, or profile;
- execution side: server, client, both, or declarative-only;
- required and optional dependencies, conflicts, and ordering constraints;
- requested capabilities and resource-budget class;
- supported platform classes and any reason portability is limited;
- exported content ids and namespaced persistent-state schema;
- content hashes, signature/provenance, and reproducible-build metadata where
  available;
- XR, touch, gamepad, keyboard/mouse, and accessibility readiness; and
- save migration, configuration schema, and removal behavior.

The portable executable artifact is canonical. Desktop JIT, browser execution,
and mobile interpretation are execution strategies for that artifact, not
package variants with different gameplay.

Trusted native/JVM extensions use a visibly different manifest class and
installation flow. A native "fast version" must not silently replace portable
logic unless equivalence is mechanically proven; otherwise it is a separate
compatibility surface.

## Profiles Are Reproducible Lockfiles

A player-facing profile or modpack is not merely an ordered list of names. It
locks:

- exact package ids, versions, payload hashes, and source registries;
- dependency resolution and deterministic load order;
- package configuration and capability grants;
- engine/API target and compatibility class;
- world-affecting schema and migration state; and
- the profile's own stable id and aggregate fingerprint.

World-affecting gameplay profiles are world or server facts. A world save
records its required profile identity and package state so opening it can
resolve, migrate, recover, or fail explicitly. Profile changes that can affect
durable state require backup/transaction semantics and declared package
removal behavior.

This differs intentionally from today's asset-pack selection:

- presentation-only asset packs remain client-global preferences;
- they do not mutate authority, saves, or protocol block-state ids;
- a gameplay package may include required assets as part of a world/server
  profile; and
- optional client presentation overrides remain personal unless the server
  explicitly requires a compatible resource for correct interaction.

Shareable profiles should produce a stable link, file, or short code and should
open into a review screen before installation. The resolver must show missing,
incompatible, untrusted, platform-limited, or permission-escalating packages
before modifying the active profile.

## In-Game Browser And Registry

The mod browser is a first-party product surface rather than an external
launcher afterthought. Useful categories include:

- gameplay and rules;
- blocks, items, recipes, and entities;
- world generation and structures;
- server and administration;
- UI and accessibility;
- textures, models, audio, and shaders;
- maps and authored worlds;
- creator tools and libraries; and
- complete profiles or modpacks.

Each package page should make the following legible before installation:

- supported game/API versions and official platforms;
- declarative, server, client, portable-script, or trusted-extension class;
- permissions and performance-budget expectations;
- dependencies, conflicts, multiplayer requirements, and save impact;
- input, touch, gamepad, XR, and accessibility support;
- source availability, license, signature, provenance, and maintainer status;
- release history, changelog, issue/support links, and compatibility reports;
  and
- an "all official platforms" badge only when automated evidence supports it.

The first-party catalog should be curated, signed, searchable, reportable, and
safe for ordinary users. The registry protocol and package format should be
open, documented, and self-hostable so communities and servers are not locked
to one service. Third-party registries and local packages remain possible but
carry a visibly different trust state.

Server-driven acquisition should resolve a profile through the same package
system. It must not bypass permission review, signature policy, age/content
controls, executable-code policy, or platform compatibility. A join should
fail clearly when a server requires an unavailable native client extension
rather than quietly degrading gameplay.

## Open-Source Relationship

A fully open first-party stack is a meaningful differentiator only when the
promise is exact. It should separately address:

- client, server, engine, tools, and launcher code licenses;
- first-party texture, model, audio, writing, and world-content licenses;
- protocol and package-format documentation;
- build reproducibility and dependency provenance;
- contribution and governance policy;
- trademark and official-build identity; and
- the boundary around proprietary store SDKs or hosted services.

Open source gives advanced creators the unlimited fork tier, preservation, and
the right to inspect the real implementation. It does not replace:

- a stable semantic API;
- compatibility and migration policy;
- safe in-game installation;
- dependency and profile resolution;
- creator documentation and examples;
- moderation and malware response; or
- polished, signed, tested official binaries.

The official catalog can curate for quality and safety while the open registry
protocol permits independent catalogs. Official signatures and trademarks
identify the supported distribution without restricting the legal ability to
fork under the eventual licenses.

## Store And Platform Policy Boundary

Sandboxing is a technical security property, not an automatic app-store
exemption.

Google Play's current policy prohibits downloading executable DEX, JAR, or
native code outside Play while describing interpreted code such as JavaScript,
Python, or Lua as conditionally allowed. It does not specifically promise that
downloaded WebAssembly gameplay modules are accepted:
[Google Play Device and Network Abuse policy](https://support.google.com/googleplay/android-developer/answer/16559646?hl=en)
and
[Android dynamic code loading risks](https://developer.android.com/privacy-and-security/risks/dynamic-code-loading).

Apple's current guidelines generally prohibit downloading or executing code
that changes app functionality. Guideline 4.7 provides a more specific path
for certain HTML5/JavaScript mini apps, games, plug-ins, and related software,
with additional catalog, moderation, privacy, purchase, and age-rating
requirements:
[Apple App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/).
Mclone has no current iOS target, but a future iOS product must not assume that
an interpreted Wasm or Lua sandbox is sufficient.

Quest is Android-based, but Google Play wording is not a Meta Horizon Store
approval. Before downloadable gameplay code becomes a Quest release promise,
obtain a written answer for the exact package, interpreter/JIT, review,
moderation, and update design under the agreement in force at submission.

Until those lanes are validated, the portable architecture can still support:

- bundled and first-party-reviewed logic;
- declarative downloaded content;
- server-side logic that stock clients do not execute;
- direct/sideloaded development builds; and
- desktop/server packages under an explicit trust policy.

Policy validation must precede a public promise of downloadable scripted mods
on a store-distributed client.

## Ownership Direction

No platform app should become the home of mod semantics.

Likely shared owners are:

- a new focused package/mod crate for manifest, dependency, capability,
  signature, registry, profile, and lockfile contracts;
- `mclone-assets` for presentation assets and compiled asset payloads;
- `mclone-server` for authoritative lifecycle, world capabilities, package
  state, and server-side script dispatch;
- `mclone-client` for replica-visible package state and bounded client
  presentation hooks;
- `mclone-protocol` for profile negotiation, package compatibility, and typed
  mod messages;
- `mclone-app-runtime` or a dedicated shared runtime crate for portable
  execution lifecycle and host-neutral operation dispatch;
- `mclone-ui` for the shared browser, permissions, profile editor, failure
  recovery, and update screens; and
- persistence owners for namespaced package state and transactional profile
  migration.

Apps may discover local paths, fetch bytes, integrate platform signing/store
metadata, provide browser storage, or schedule native versus worker execution.
They do not define different package semantics, permissions, host APIs, or
profile resolution.

## Validation Contract

The eventual portable runtime and package system need tests at several layers:

- host-API conformance fixtures shared by interpreter, JIT, AOT, and browser
  execution engines;
- identical behavior and serialized authoritative results for deterministic
  fixtures across desktop, web, Android, and Quest;
- fuel, memory, queue, cancellation, watchdog, and malicious-package tests;
- dependency, conflict, signature, capability-escalation, rollback, and
  migration tests;
- world reopen with exact, upgraded, missing, removed, and corrupt profiles;
- server join with matching, resolvable, incompatible, untrusted, and
  native-client-required profiles;
- integrated and dedicated server parity;
- client-global visual pack composition with a world-bound gameplay profile;
- renderer and UI validation across flat, touch, controller, and XR clients;
  and
- store-policy receipts for every channel that downloads executable gameplay
  behavior.

JIT-versus-interpreter tests must demonstrate semantic equivalence. Performance
benchmarks should measure cold start, steady tick cost, memory per module, host
call overhead, package size, and worst-case budget enforcement.

## Research And Implementation Sequence

1. Define ten to fifteen representative mods before choosing a runtime:
   a recipe/block pack, crop mechanic, entity behavior, bulk world operation,
   world generator, quest system, server administration plugin, declarative
   UI, client presentation effect, and one deliberately hostile workload.
2. Derive the smallest semantic host-capability API that supports those cases
   without exposing engine internals.
3. Spike the same bounded gameplay package in WebAssembly and one strict Lua
   profile across native desktop, browser, Android, Quest, and dedicated
   server. Measure startup, memory, tick cost, host calls, package size,
   debugging, interruption, and fault containment.
4. Obtain written store-policy answers for downloaded declarative content,
   interpreted Lua, interpreted WebAssembly, JIT, network acquisition, and
   server-required packages. Keep exact questions and receipts.
5. Specify package manifests, signatures, stable ids, dependency resolution,
   capability grants, and reproducible profile lockfiles by extending rather
   than replacing proven asset-pack concepts.
6. Implement local developer packages and deterministic profile resolution
   before building a hosted public catalog.
7. Prove integrated/dedicated authority, browser worker execution, save
   migration, server join, rollback, and cross-platform conformance.
8. Add the shared in-game browser against a development registry, then define
   curation, moderation, federation, creator identity, and public operations.
9. Consider JVM/native trusted-extension adapters only after the portable
   ecosystem and its incompatibility signaling are stable.

## Accepted Decisions

- One package ecosystem spans official targets; Mclone does not intentionally
  create separate Java-like and Bedrock-like editions.
- Declarative content is the broadest and safest extension tier.
- Ordinary gameplay mods use a portable capability sandbox and
  server-authoritative rules where practical.
- WebAssembly is the preferred runtime boundary to prototype; the final
  runtime choice follows evidence rather than this document alone.
- Interpretation is the semantic baseline. JIT or AOT is an optimization, not
  a feature or compatibility tier.
- If Lua is selected, Mclone defines one strict profile and does not pair
  unrelated mobile and desktop language semantics.
- The initial product supports one general-purpose portable scripting runtime,
  not parallel Lua and WebAssembly ecosystems.
- Native Rust, JVM, FFI, and arbitrary host access belong to explicitly
  trusted server/desktop extensions or source forks.
- Gameplay profiles are exact, reproducible, world/server-bound lockfiles.
- Client-global presentation packs remain distinct from world-affecting
  gameplay profiles.
- The mod browser, permissions UI, profile manager, rollback, and diagnostics
  are core product surfaces.
- The package and registry protocols should be open and self-hostable even if
  Mclone operates a curated default catalog.
- Fully open source strengthens the unrestricted fork path but does not replace
  a stable mod API, distribution UX, sandbox, or moderation.
- Downloadable scripting on store clients is not promised until the exact
  channel policy has been validated.

## Open Decisions

- WebAssembly versus a strict Lua profile after the cross-platform spike.
- The first creator-facing language and debugging/tooling experience if Wasm
  is the binary contract.
- Core WebAssembly plus a custom ABI versus fuller component-model adoption.
- Runtime implementation per native platform and the cost of shipping more
  than one execution backend.
- The exact host API, capability taxonomy, budgets, and versioning policy.
- Which client-side script capabilities are required beyond declarative UI,
  effects, and server messages.
- Whether shaders are declarative/validated assets, a separately reviewed
  capability class, or excluded from portable packages.
- Registry federation, creator identity, signatures, review, reporting,
  ratings, discovery, and abandoned-package policy.
- Free, donation-supported, or paid package policy and its interaction with
  open content/source licenses.
- Save migration and recovery UX when a required package disappears or becomes
  incompatible.
- Server auto-acquisition policy and whether clients may approve permissions
  once per package, profile, server, or world.
- Exact code, content, contribution, trademark, and official-build licenses.
- Written Google, Meta, and any future Apple rulings for the intended
  executable-content flow.

## Related Code And Documents

- [`asset-pack-profiles.md`](asset-pack-profiles.md): current stable ids,
  manifests, roles, provenance, fingerprints, shared selection UI, and
  transactional application concepts.
- [`multiplayer-networking.md`](multiplayer-networking.md): authoritative
  session and transport-neutral protocol direction.
- [`world-generation-profiles.md`](world-generation-profiles.md): versioned
  generator identity and compatibility safety.
- [`unified-persistence-interface.md`](unified-persistence-interface.md):
  cross-platform durable-operation boundary needed by package state and
  migrations.
- [`cross-platform-operation-execution.md`](cross-platform-operation-execution.md):
  shared native-thread/browser-worker operation execution.
- [`platform-parity.md`](platform-parity.md): all-target feature and
  shared-contract parity tracking.
- [`release-distribution-and-updates.md`](release-distribution-and-updates.md):
  signed first-party distribution and update ownership.
- [`distribution-go-to-market.md`](distribution-go-to-market.md): open-stack
  positioning and store/channel decisions.
- [`voxel-sandbox-competitive-landscape.md`](voxel-sandbox-competitive-landscape.md):
  creator-ecosystem and open-source comparison evidence.
- [`../../native/crates/mclone-assets`](../../native/crates/mclone-assets):
  current asset-pack contracts.
- [`../../native/crates/mclone-server`](../../native/crates/mclone-server):
  authoritative simulation owner.
- [`../../native/crates/mclone-protocol`](../../native/crates/mclone-protocol):
  future profile negotiation and typed package-message owner.
- [`../../native/crates/mclone-ui`](../../native/crates/mclone-ui): future
  shared mod-browser and profile-management UI owner.
