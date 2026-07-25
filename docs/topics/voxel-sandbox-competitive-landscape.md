# Voxel Sandbox Competitive Landscape

Topic: `voxel-sandbox-competitive-landscape`

Status: **initial product-research guide recorded 2026-07-24.** This is a
dated orientation map, not a final market ranking or an accepted feature
roadmap. It deliberately combines direct voxel competitors, important
Minecraft mods and access routes, and adjacent games that are unusually good
at one part of the survival/building proposition. Product status, platforms,
prices, reviews, and video availability must be rechecked before commercial
decisions.

Last reconciled: **2026-07-24**.

## Scope

This topic exists to help a maintainer become familiar with the broader
Minecraft-like and voxel-sandbox landscape, see what each product does
unusually well, watch representative gameplay, and turn inspiration into
clear Mclone product questions.

It owns:

- a taxonomy of direct competitors, adjacent substitutes, platforms, mods,
  and technical or interaction references;
- concise profiles of the strongest products and their signature qualities;
- official pages, representative official videos, and deliberately broad
  YouTube gameplay searches;
- a dated comparison of source availability, content licensing, mod APIs,
  creator tooling, and player-facing content distribution;
- an inspiration ledger: what looks impressive, what to watch closely, and
  what question it creates for Mclone;
- a wider scan list and a watchlist for products that merit periodic review;
  and
- the current hypothesis about Mclone's distinctive *combination* of
  capabilities.

It does **not** own:

- pricing, storefront priority, launch sequencing, entitlement, or commercial
  policy; those remain in
  [`distribution-go-to-market.md`](distribution-go-to-market.md);
- implementation priority or platform completeness; those remain in
  [`platform-parity.md`](platform-parity.md) and the relevant subsystem topic;
- an instruction to copy another game's proprietary code, assets, characters,
  trade dress, progression, or exact content;
- quantitative audience, revenue, review-volume, or pricing claims without a
  separately dated market-data pass; or
- a claim that every game using voxels competes for the same player.

## How To Use This Guide

Trailers reveal the intended fantasy and strongest visual claim. They do not
show ordinary cadence, friction, UI, inventory work, repetition, server setup,
or what a world feels like after ten hours. For a serious review:

1. Read the short profile and official page.
2. Watch the official video to learn the product's chosen promise.
3. Use the gameplay-search link to find a recent, minimally edited first hour
   or long-form session. Add the current year or version when necessary.
4. Watch one advanced build, settlement, server, automation, or endgame tour.
5. Record observations in the review template below rather than converting
   first impressions directly into feature requests.

For every product, ask:

- What fantasy is legible in the first fifteen seconds?
- What happens in the first thirty minutes, after ten hours, and after one
  hundred hours?
- Why does the player build: safety, expression, production, status, story,
  settlement growth, defense, or social belonging?
- Does the world react systemically, appear alive decoratively, or remain a
  passive resource field?
- Which interactions feel intrinsically satisfying even without progression?
- What creates goals without destroying sandbox freedom?
- How easily can a friend join, return without the owner, or contribute in a
  different role?
- Which creator actions happen inside the game, and which require external
  tools or mod installation?
- Can a player legally inspect, build, modify, fork, and redistribute the
  client, server, tools, and first-party content, or is the source merely
  visible?
- Is modding a supported product path with stable APIs, documentation,
  dependency handling, discovery, installation, updates, and multiplayer
  behavior, or only something technically possible?
- How much of the experience survives touch, controller, handheld, browser,
  or XR constraints?
- What screenshot, ten-second clip, or player story could belong only to this
  product?

The YouTube search links are discovery aids, not endorsements of whichever
creator or result is ranked first. Search results and videos change over time.
Prefer current unedited play over commentary that only repeats a store page.

## The Market Is Several Genres At Once

"Minecraft-like" hides materially different player promises:

| Player promise | Representative leaders or references | The central question |
|---|---|---|
| General survival/build/explore sandbox | Minecraft, Vintage Story, Hytale, Survivalcraft 2, Creativerse, VoxeLibre, Lay of the Land | Is the world worth inhabiting after novelty fades? |
| Authored voxel adventure or RPG | Dragon Quest Builders 2, Hytale, Portal Knights, Veloren, Block Story | Can story and progression give building purpose without making it decorative? |
| Society, ecology, and colony simulation | Eco, Colony Survival, Terasology, MineColonies | Do player construction and resource choices change a living society or environment? |
| Engineering, automation, and functional bases | Minecraft Create, Factorio, FortressCraft Evolved, Space Engineers | Can a build *do* something interesting and become a reusable system? |
| Destruction and physically reactive material | 7 Days to Die, Lay of the Land, Teardown, Space Engineers | Does changing the world have weight, risk, and surprising consequences? |
| Persistent social and creator platform | Minecraft servers, LEGO Fortnite Odyssey, Hytale, Luanti, Roblox, Boundless, Trove | How quickly does authored player activity become shared culture? |
| Mobile and low-friction block sandbox | Minecraft Bedrock, Survivalcraft 2, Luanti, ClassiCube, browser block games | What remains pleasant under touch, small screens, and short sessions? |
| Native XR and spatial building | Discovery 2, cyubeVR, QuestCraft, Vivecraft | Does embodiment create a better interaction or merely add physical effort? |
| Adjacent progression/retention substitute | Terraria, Core Keeper, Valheim, Enshrouded | What makes a group return when blocks themselves are not the main attraction? |

The useful comparison is therefore not "which clone has the most Minecraft
features?" It is "which product most convincingly owns each player promise,
and which combination can Mclone credibly own?"

## Strategic Synthesis

The strongest current references each have a clear center:

| Product or ecosystem | Especially strong center |
|---|---|
| Minecraft Java/Bedrock | Category ownership, breadth, cultural literacy, servers, mods, creators, and long-lived worlds |
| Vintage Story | Materially grounded survival, manual crafts, geology, seasons, and a coherent harsh-world identity |
| Hytale | Voxel RPG presentation, creator tools, modding, player-run servers, and visible content ambition |
| LEGO Fortnite Odyssey | Accessible cross-platform social survival backed by a large identity and account ecosystem |
| Eco | Player specialization, economy, government, ecology, and consequences that require coordination |
| Dragon Quest Builders 2 | Authored story and characters that teach, motivate, and celebrate building |
| Colony Survival | First-person construction joined to large-scale NPC labor, technology, and nightly defense |
| Factorio / Create | Machines, throughput, logistics, debugging, and builds whose function creates the next goal |
| 7 Days to Die | Construction under recurring pressure in a destructible world |
| Lay of the Land / Teardown | Voxel material used for physics, simulation, and emergent destruction rather than only block placement |
| Discovery 2 / cyubeVR | Spatial manipulation and VR-native building interaction |
| Luanti and its games | Free, open, moddable voxel infrastructure and in-client content discovery |
| Survivalcraft 2 | Compact paid mobile survival with touch-first roots and a more naturalistic animal/survival emphasis |
| Enshrouded | High-fidelity voxel terrain and detailed building inside a broad cooperative action-RPG offer |

Mclone should not claim uniqueness merely because it has survival, procedural
terrain, multiplayer, mods, XR, or a web build. Strong products already own
each ingredient.

The more defensible hypothesis is the combination:

- one shared persistent game across desktop, Steam Deck, web, flat Android,
  desktop OpenXR, and standalone Quest;
- instant entry into the full client from a browser link;
- first-class flat, touch, controller, and tracked-controller interaction;
- one living world usable both embodied and as a manipulable overview model;
- live embedded destinations and world previews rather than static menu cards;
- an AI-authored structure catalogue whose pages are walkable in the actual
  browser client;
- a candidate fully open first-party stack: client, server, shared engine,
  tools, protocols, and redistributable original content rather than only a
  readable server or a proprietary game with a mod API; and
- an original world and identity rather than public Minecraft compatibility.

No reviewed product currently demonstrates that full bundle. That is a
positioning hypothesis, not proof of demand. The individual pieces must feel
good enough that the integration is visible to a player rather than only
impressive in an architecture diagram.

## Source Openness And Modding Are Separate Axes

This comparison was checked on **2026-07-24**. It is product research, not
legal advice. Licenses, repositories, creator programs, and mod policies can
change; inspect the actual license text and current distribution agreement
before making a dependency or product commitment.

### Use Precise Terms

- **Open source** means code is under a recognized license that permits use,
  modification, and redistribution. The
  [Open Source Definition](https://opensource.org/osd) is the useful baseline.
  A public repository or permission to read code does not by itself qualify.
- **Source available** or **readable source** means some code can be inspected
  under a custom or restrictive license. It may still prohibit redistribution,
  unrelated forks, commercial use, or a competing game.
- **Moddable** means a game exposes a supported extension path. A closed game
  can have excellent modding; an open game can have no stable package API,
  discovery surface, or compatibility policy.
- **UGC-capable** means players can share worlds, maps, prefabs, skins, or
  experiences. UGC may be powerful while the engine and runtime remain closed.
- **Open content** is separate from open code. Textures, models, music, sounds,
  writing, fonts, and world data need explicit content licenses such as CC0,
  CC BY, or CC BY-SA if they are intended to be reusable.
- **Full first-party stack** should mean all first-party code and releasable
  content needed to build, modify, host, and play the game: clients, dedicated
  server, shared engine/runtime, tools, schemas, protocols, build scripts,
  first-party gameplay data, and original assets. Trademarks, signing keys,
  platform SDKs, account services, moderation systems, and hosted
  infrastructure can remain separately controlled or replaceable.

That last phrase is the strongest plausible Mclone claim, but it is not the
project's current legal state. As of this audit, the repository has no
project-level license. Publicly readable source without an explicit license is
normally not permission to copy, modify, or redistribute it. Treat full
openness as a candidate strategy requiring an affirmative license and
governance decision. “Everything” can only mean everything Mclone has the
right to license: the decompiled Minecraft reference/oracle inputs, upstream
libraries, platform SDKs, fonts, and other third-party materials retain their
own terms and must remain clearly separated from the open first-party product.

### Deep-Profile Source And Mod Posture

This table covers every product family given a deep profile below. “No
first-party path identified” is deliberately narrower than claiming that no
unofficial mod exists.

| Product or family | Code and content posture | Supported extension or sharing story | Main lesson for Mclone |
|---|---|---|---|
| Minecraft Java / Bedrock | Proprietary clients, server distributions, and first-party assets governed by the [Minecraft EULA](https://www.minecraft.net/en-us/eula) and usage guidelines | Java has an enormous community ecosystem built around third-party loaders and server APIs; Bedrock has first-party [creator documentation](https://learn.microsoft.com/en-us/minecraft/creator/), add-ons, packs, scripting, and Marketplace distribution | Ecosystem scale can outweigh a clean official API. Mclone cannot outcatalogue Minecraft early, but it can make first-party source, builds, self-hosting, and one cross-device extension contract far less ambiguous |
| Vintage Story | Proprietary game. Significant API and gameplay modules are published as readable source, but the [VS API repository license](https://github.com/anegostudios/vsapi) says the software remains proprietary and limits its use to learning and Vintage Story mods | Modding is a first-class architecture: JSON content, C# code, survival itself structured as a mod, API docs, tutorials, sample mods, server-side paths, and an official [mod database](https://mods.vintagestory.at/). The official [modding overview](https://www.vintagestory.at/old/features/modding.html/) is unusually candid | This is the closest direct benchmark for a coherent commercial survival game with deep built-in modding. Mclone would differentiate on legal forkability and the complete stack, not merely “we expose an API” |
| Hytale | Closed common client. The studio describes a shared-source Java server direction and has promised server source access, but its detailed [modding status](https://hytale.com/news/2025/11/hytale-modding-strategy-and-status) also makes clear that client mods are intentionally excluded | Server-first Java plugins, JSON data, art packs, worlds, prefabs, asset editors, Blockbench integration, creative tools, planned visual scripting, and automatic server content acquisition | A visible or shared server plus excellent tools is not the same as an open game. Mclone can be more forkable, while Hytale is a major benchmark for making creator power approachable and multiplayer-safe |
| LEGO Fortnite Odyssey | Proprietary game and Fortnite platform | [UEFN LEGO island templates](https://dev.epicgames.com/documentation/en-us/fortnite/creating-lego-islands-in-fortnite) and Fortnite Creative provide a governed UGC path inside Epic's platform rather than engine or game forks | Strong reach, discovery, social identity, and creator tooling can exist without source openness. An open Mclone still needs equally legible publishing and joining |
| Eco | Proprietary game and distributed server, with documented APIs rather than an open first-party stack | C# server mods, configuration, a freely available [ModKit](https://wiki.play.eco/en/Installing_the_ModKit), API documentation, mod.io/community distribution, and server delivery of relevant client content; the official [mod development page](https://wiki.play.eco/en/Mod_Development) warns that APIs can still break | Server-authoritative mod delivery is excellent for coordinated worlds. Open source would add inspectability and fork continuity, but does not replace API stability or creator support |
| Dragon Quest Builders 2 | Proprietary game and authored assets | Shareable builds and community islands are central; no first-party code-mod or engine-extension path was identified in the official material reviewed | A narrow sharing format can still create inspiration and retention. Mod breadth is not required for every successful building game |
| Colony Survival | Proprietary commercial game | The official [product page](https://www.colonysurvival.nl/) advertises mods, Steam Workshop, blueprints, and co-op | Mod and blueprint support strengthens a systems-heavy game, but source openness would be a separate promise |
| 7 Days to Die | Proprietary commercial game | Longstanding XML/data customization, modlets, code mods, dedicated-server configuration, and an active [official modding forum](https://community.thefunpimps.com/categories/game-modification.48/) | Editable data and server ownership can sustain large unofficial ecosystems even without an open engine. Version migration and compatibility remain the cost |
| Lay of the Land | Proprietary Early Access game | No first-party mod API or creator distribution path was identified in the official material reviewed | Deep physical simulation is not automatically an extensible platform. Opening implementation code could be valuable, but safe semantic hooks would still be needed |
| Luanti / VoxeLibre / Mineclonia | Luanti is an LGPL-2.1+ open engine. Its official [licensing guide](https://docs.luanti.org/for-creators/licensing/) explains the engine boundary; individual games and assets carry their own free licenses and must be checked package by package | Lua games and mods are fundamental to the architecture. [ContentDB](https://docs.luanti.org/about/contentdb/) is available in-client, only accepts freely licensed packages, manages updates, and supports games, mods, and texture packs. Server-side mods require no separate client install | This is the strongest direct proof that open, moddable voxel infrastructure already exists. Mclone needs a more cohesive authored product, strong presentation, and a distinct cross-device/XR experience as well as openness |
| Survivalcraft 2 | Proprietary commercial game | World sharing, custom content, and community artifacts exist, but no open client/server or comparable first-party code-mod API was identified in this pass | A focused mobile product can survive through compact design and shareable worlds rather than broad programmability |
| Discovery 2 / cyubeVR / QuestCraft | Discovery 2 and cyubeVR are proprietary games. QuestCraft is a public community compatibility project, but it still requires the proprietary Minecraft Java game and assets | Discovery 2 has an in-client world gallery. cyubeVR has custom blocks, a native VoxelAPI, Unreal Blueprint mods, and [Steam Workshop integration](https://store.steampowered.com/news/posts/?appgroupname=cyubeVR&appids=619500&enddate=1680879223&feed=steam_community_announcements). QuestCraft preserves much of Java Minecraft's mod ecosystem on standalone Quest | XR does not imply a weak mod story. cyubeVR is a particularly important benchmark for creator tooling in a tracked-controller game; QuestCraft shows that open glue around a closed dependency is not a fully open product |
| Enshrouded | Proprietary game on a proprietary engine | Dedicated servers and shared worlds exist. Its current [official FAQ](https://enshrouded.com/en-US/faq) says mod support is desired but not promised | High visual quality and world sharing can ship before a mod platform, but this leaves a clear openness and extensibility contrast |

### Broader Open And Modding Benchmarks

The wider scan contains additional control cases that prevent an overly simple
“open versus closed” conclusion:

| Reference | Source posture | Mod or contribution posture | What to study |
|---|---|---|---|
| [Terasology](https://github.com/MovingBlocks/Terasology) | Fully open: Apache-2.0 code and normally CC BY 4.0 artwork | Modular multi-repository engine/game architecture and direct community contribution | The strongest explicit “open code plus open art” comparator; also study the onboarding, cohesion, release cadence, and product-polish costs of a broad volunteer platform |
| [Veloren](https://gitlab.com/veloren/veloren) | GPL-3.0-or-later code, with asset attribution and license metadata tracked in the repository | Direct contribution and forks are mature; a consumer-facing package-mod catalogue is less central than the open project itself | Open development can produce an original, visually distinctive multiplayer voxel RPG. Source access is not the same UX as installing a compatible mod |
| [ClassiCube](https://github.com/ClassiCube/ClassiCube) | Publicly buildable clean-room client whose main code uses BSD 3-Clause terms; original Minecraft assets remain an external dependency | Client/server extensions and direct forks, plus exceptional platform portability | Open code can unlock web, mobile, old hardware, and homebrew ports, while external proprietary content can still limit the completeness of the open promise |
| [Factorio](https://wiki.factorio.com/Modding) | Proprietary game | Stable Lua API, data stages, in-game mod management, dependency metadata, version compatibility, and an official [mod portal/API](https://wiki.factorio.com/Mod_portal_API) | Perhaps the clearest proof that a closed game can offer a better player-facing mod product than many open games |
| [Teardown](https://www.teardowngame.com/) | Proprietary game | Lua modding, maps, tools, vehicles, total experiences, Steam Workshop, and an in-game mod loader | Immediate remixability and distribution make physical simulation into a creator platform |
| [Space Engineers](https://www.spaceengineersgame.com/modding-guides/) | Proprietary game; published reference source must not be confused with an OSI-open game | C# scripting/mod APIs, SDK guidance, mod.io/Steam Workshop, blueprints, worlds, and server plugins | Readable source, powerful mods, and Workshop content are three separate layers even when players experience them as one ecosystem |

Two conclusions can both be true:

1. **“Open-source voxel game” is not unique.** Luanti games, Terasology,
   Veloren, ClassiCube, and other public projects already occupy that ground.
2. **A polished commercial-scale survival game with a fully open first-party
   stack across native desktop, browser, Android, standalone XR, desktop XR,
   and dedicated server would be unusual.** That combination is the sharper
   differentiator, especially if the original assets and build/release path
   are genuinely reusable rather than only the engine repository being public.

The public promise should therefore avoid “the first open-source Minecraft
alternative.” A more defensible future claim would be:

> The complete first-party game stack is open: build, modify, fork, and
> self-host the client, server, tools, protocols, gameplay, and redistributable
> original content. Official releases remain the easiest, curated way to play.

That wording remains a hypothesis until the actual licenses, dependency audit,
asset provenance, platform exceptions, and reproducible-build path make it
true.

### Open Source Does Not Create A Mod Product

Forking the whole game is powerful for preservation, research, accessibility,
new platforms, total conversions, and experiments. It is a poor substitute for
installing one compatible feature into an existing world. Mclone needs both
layers if it wants openness to be felt by ordinary players:

- a documented, versioned data and code extension contract rather than
  requiring patches to engine internals;
- one package manifest covering identity, dependencies, target versions,
  client/server needs, permissions, license, provenance, and content hashes;
- in-client discovery, installation, updates, disable/rollback, and
  world-specific mod sets across flat, touch, controller, web, and XR clients;
- server-declared content with an understandable join flow, bounded download,
  verification, and a safe sandbox for untrusted logic;
- stable content registries, namespacing, save migrations, missing-content
  behavior, and compatibility diagnostics;
- documented examples and the same tools/contracts used by first-party
  gameplay rather than a weaker parallel API;
- distribution that preserves author attribution and source/license links;
  and
- a policy for paid mods, donations, forks, abandoned packages, malicious
  content, moderation, and multiplayer anti-cheat.

The strongest ecosystem examples combine several of these. Luanti makes
content discovery and server use easy; Vintage Story makes first-party
gameplay an example mod; Factorio makes compatibility and dependencies
legible; Hytale emphasizes server-first automatic acquisition; Minecraft
contributes unmatched community scale; Teardown and cyubeVR connect creation
to a workshop.

### Decisions Required Before Calling Mclone Fully Open

“Everything open” needs an explicit boundary and cannot be inferred from
putting the current repository on a public forge:

1. **Code license:** choose a standard permissive or copyleft family for the
   client, server, shared crates, tools, web client, and platform adapters.
   Decide whether proprietary forks, linked store SDKs, and hosted modified
   servers must publish changes. Do not invent a custom “open but not for
   competitors” license and still call it open source.
2. **Content license:** separately license first-party textures, models,
   sounds, music, fonts, writing, worlds, and structure catalogue artifacts.
   Complete a provenance audit first; some dependencies may only permit
   redistribution, not remixing. Minecraft reference source/assets and any
   derived test inputs must not be presented as open Mclone content.
3. **Build completeness:** publish manifests, generators, schemas, asset
   compilers, CI/release recipes, and substitute interfaces for proprietary
   platform SDKs. A source tree that cannot produce a usable game is a weaker
   promise.
4. **Protocol and self-hosting:** document network and world formats, ship the
   dedicated server, and make accounts, discovery, and hosted services
   replaceable or optional where practical.
5. **Trademark and official distribution:** keep the name, logos, signing
   keys, safety claims, and official service identity under a clear trademark
   policy. Open source does not require unofficial forks to impersonate the
   official product.
6. **Contribution governance:** define decision rights, contribution terms,
   review expectations, a code of conduct, security disclosure, and how
   long-lived forks or upstream contributions work. Obtain legal review of the
   final scheme.
7. **Commercial value:** make paid Steam, Quest, and other store editions sell
   convenience, tested binaries, automatic updates, platform integration,
   support, and confidence—not artificial source scarcity. Keep hosted
   services and official content clearly distinguishable from the right to
   build and self-host.

This landscape does not choose permissive versus copyleft licensing. That
decision changes fork incentives, proprietary platform integration, hosted
server obligations, contributor expectations, and commercial positioning, so
it deserves a dedicated product/legal decision with a dependency inventory in
hand.

Open code does not make authoritative multiplayer impossible. Cheats already
operate against closed clients; server authority, validation, rate limits,
permissions, signatures, and moderation remain the relevant controls. It does
make security theater based on obscurity less available, which is a healthy
constraint if the architecture is designed accordingly.

## Deep Profiles

### Minecraft Java And Bedrock

**Why study it:** Minecraft is the baseline language players already know, not
just another row in a comparison table. Java and Bedrock also demonstrate
different product tradeoffs: open-ended community modification and servers on
one side; broad device reach, integrated marketplace/account services, and
controller/touch support on the other.

**Impressive or distinctive aspects:**

- Breaking and placing one-meter blocks remains an exceptionally legible,
  low-rule creative verb.
- Survival progression, redstone, farms, exploration, building, servers,
  minigames, education, commands, and mods coexist without one official
  endgame owning the product.
- A tiny visual and interaction vocabulary generates culturally recognizable
  player stories and artifacts.
- Seeds, builds, skins, maps, videos, servers, and mods are independent
  acquisition and retention loops.
- Long-lived worlds accumulate personal meaning even when individual systems
  are mechanically shallow.

**Watch:**

- [Official Minecraft overview](https://www.minecraft.net/en-us/about-minecraft)
- [Official Minecraft YouTube channel](https://www.youtube.com/@minecraft)
- [Recent survival first-hour gameplay search](https://www.youtube.com/results?search_query=Minecraft+survival+first+hour+gameplay+2026)
- [Advanced long-term world tour search](https://www.youtube.com/results?search_query=Minecraft+long+term+survival+world+tour)

**Watch for:** how little explanation basic world editing needs; how quickly
players invent self-directed goals; the gap between a fresh vanilla world and
a socially or technically mature one; where UI and inventory friction have
become normalized only because the game is familiar.

**Mclone question:** which parts of Minecraft literacy should transfer
immediately, and which familiar assumptions should Mclone deliberately break
to establish its own identity?

### Vintage Story

**Why study it:** Vintage Story is the most important direct comparator for a
premium original voxel survival game whose identity is depth rather than
Minecraft-scale breadth.

**Impressive or distinctive aspects:**

- Knapping, clay forming, casting, smithing, food preparation, and other
  crafts make transformation feel physical rather than like selecting a grid
  recipe.
- Rock strata, large mineral deposits, prospecting, caves, and ore processing
  make geology part of the survival loop.
- Seasons, localized weather, farming, food preservation, clothing, and
  temperature make time and preparation meaningful.
- Mechanical power, animal husbandry, storage, and homestead work create a
  long domestic arc after immediate survival.
- Temporal instability, ruins, and horror elements give the naturalistic
  survival systems an authored uncanny identity.
- Integrated mods, a public server list, customization, and direct
  distribution support a focused long-tail community.

**Watch:**

- [Official survival feature overview](https://www.vintagestory.at/old/features/survival.html/)
- [Official feature trailer](https://www.youtube.com/watch?v=NJjifFq1NGY)
- [Recent unedited first-year gameplay search](https://www.youtube.com/results?search_query=Vintage+Story+first+year+unedited+gameplay+2026)
- [Advanced homestead and mechanical-power tour search](https://www.youtube.com/results?search_query=Vintage+Story+advanced+homestead+mechanical+power+tour)

**Watch for:** which manual processes remain satisfying after repetition;
whether realism creates meaningful decisions or merely longer recipes; how
seasonal deadlines change priorities; how sound, darkness, weather, and
animation contribute more than the feature list.

**Mclone question:** can Mclone make materials, terrain, seasons, and
preparation feel consequential without inheriting all of Vintage Story's
deliberate friction and harshness?

### Hytale

**Why study it:** Hytale is the strongest current benchmark for presenting a
voxel game as a polished fantasy RPG, creator platform, and server ecosystem
at the same time.

**Impressive or distinctive aspects:**

- Responsive combat, creatures, dungeons, props, animation, and an original
  art direction make the block world read as an authored adventure.
- Player-run servers and moddable worlds are part of the central proposition
  rather than a late extension.
- Shared-source server plans and integrated server discovery lower the barrier
  to community-owned experiences.
- Blockbench integration, prefab tools, sculpting, trigger volumes, camera
  control, and in-game tooling broaden creation beyond placing individual
  blocks.
- The tools used to build the game are deliberately exposed as a product
  surface.

**Watch:**

- [Official game and tools overview](https://hytale.com/game)
- [Official announcement trailer](https://www.youtube.com/watch?v=o77MzDQT1cg)
- [Official trigger-volume demonstration](https://www.youtube.com/watch?v=quescI46kEw)
- [Current Early Access gameplay search](https://www.youtube.com/results?search_query=Hytale+Early+Access+unedited+gameplay+2026)
- [Current creative-tools and server tour search](https://www.youtube.com/results?search_query=Hytale+creative+tools+server+tour+2026)

**Watch for:** whether adventure content remains interesting beside creator
tools; how much visual richness comes from non-block props and animation; how
quickly a player can move from consumer to creator; whether server discovery
creates a coherent or fragmented product identity.

**Mclone question:** if Hytale owns "voxel RPG plus creator platform," what
does Mclone show in its first minute that is clearly not a smaller version of
that promise?

### LEGO Fortnite Odyssey

**Why study it:** LEGO Fortnite Odyssey is a major benchmark for approachable,
cross-platform survival played through an existing social graph, identity,
parental-control, and content platform.

**Impressive or distinctive aspects:**

- Survival, Sandbox, Cozy, and Expert configurations present the same world
  fantasy to players with different tolerance for pressure.
- Eight-player private worlds, key-holder access while the owner is offline,
  and broad cross-play make a shared world easy to treat as a social place.
- LEGO pieces, prefabs, recognizable characters, animation, and physical-to-
  digital brand continuity make building emotionally legible before systems
  are explained.
- Village growth and helpful characters make construction feel inhabited.
- Fortnite accounts, friends, platform reach, live operations, and discovery
  remove acquisition problems an independent game must solve itself.

**Watch:**

- [Official parent and product overview](https://www.lego.com/en-us/article/lego-odyssey-parents-guide)
- [Official survival getting-started guide](https://www.lego.com/en-us/themes/fortnite/get-started-lego-fortnite-survival-mode)
- [Recent first-hour gameplay search](https://www.youtube.com/results?search_query=LEGO+Fortnite+Odyssey+first+hour+gameplay+2026)
- [Advanced village and shared-world tour search](https://www.youtube.com/results?search_query=LEGO+Fortnite+Odyssey+advanced+village+world+tour+2026)

**Watch for:** how quickly a friend can join; how prefab placement and
freeform building coexist; what village NPCs contribute; how the game
communicates safe/cozy versus expert play; how much retention comes from the
survival world versus the surrounding Fortnite platform.

**Mclone question:** can a no-install browser link and mixed-device session
approach LEGO Fortnite's social convenience without requiring an equally huge
account ecosystem?

### Eco

**Why study it:** Eco is the strongest reference for making a voxel world a
shared ecological, economic, and political problem rather than an infinite
pile of consequence-free resources.

**Impressive or distinctive aspects:**

- Species, habitat, soil, pollution, climate, and resource extraction
  participate in an ecosystem simulation with visible data.
- Specializations make players mutually dependent instead of letting every
  participant silently complete the same technology tree.
- Stores, currencies, contracts, labor, property, and taxation make exchange
  part of play rather than an external server convention.
- Constitutions, elections, districts, laws, and public data turn conflicting
  incentives into explicit multiplayer mechanics.
- The meteor creates a shared deadline and victory pressure, while server
  settings can scale collaboration for different group sizes.
- Buildings, roads, farms, industry, and public works become evidence of a
  society rather than only personal bases.

**Watch:**

- [Official overview](https://www.play.eco/)
- [Official “Tour of an Eco Society”](https://www.youtube.com/watch?v=DKX34ULV7Ps)
- [Recent fresh-server gameplay search](https://www.youtube.com/results?search_query=Eco+game+fresh+server+gameplay+2026)
- [Mature civilization and government tour search](https://www.youtube.com/results?search_query=Eco+game+mature+society+government+economy+tour)

**Watch for:** how much cooperation emerges naturally versus being enforced
by progression costs; whether ecological feedback is perceptible without map
graphs; how newcomers find a useful role in an established society; what
happens when players stop logging in.

**Mclone question:** what is the smallest living-world simulation whose
consequences players can understand through ordinary play, without requiring
Eco's full government and economy stack?

### Dragon Quest Builders 2

**Why study it:** Dragon Quest Builders 2 is an unusually successful answer
to "how can authored story teach and motivate freeform building?"

**Impressive or distinctive aspects:**

- A charming campaign, companion character, villages, quests, and explicit
  building problems give construction emotional and narrative purpose.
- Blueprints and villagers turn a desired building into a legible shared
  project.
- New materials, recipes, traversal abilities, farming, and locations arrive
  through a paced authored journey.
- The game celebrates completed rooms and settlements, giving feedback beyond
  the player's private aesthetic judgment.
- A separate home island and multiplayer building mode preserve expression
  after guided content.
- Third-person animation and readable characters make the builder part of the
  scene rather than a floating hand.

**Watch:**

- [Official product overview](https://na.store.square-enix-games.com/dragon-quest-builders-2)
- [Official Steam launch trailer](https://www.youtube.com/watch?v=vDjiOBfxnrQ)
- [Campaign first-hour gameplay search](https://www.youtube.com/results?search_query=Dragon+Quest+Builders+2+first+hour+gameplay)
- [Late-game island and villager build tour search](https://www.youtube.com/results?search_query=Dragon+Quest+Builders+2+late+game+island+build+tour)

**Watch for:** tutorial density; how blueprints preserve agency; how NPC
reactions make spaces feel valid; the balance between authored islands and
the player's home; which building affordances reduce block-by-block fatigue.

**Mclone question:** can settlements, guided builds, and the structure
catalogue provide purpose and celebration without requiring a linear JRPG
campaign?

### Colony Survival

**Why study it:** Colony Survival directly combines first-person voxel
building with colony-scale labor, production, technology, logistics, and
defense.

**Impressive or distinctive aspects:**

- Players design towns, castles, walls, farms, workshops, routes, and defenses
  that thousands of colonists can use.
- Recruitment and job assignment turn built space into a production system.
- A technology tree spanning the Stone Age to industrial machinery gives
  settlements a long arc.
- Nightly monster pressure makes walls, traps, weapons, and settlement layout
  functional.
- Co-op, controller/Deck support, mods, blueprints, trains, aircraft, and
  large cities widen the endgame beyond personal survival.
- The first-person viewpoint preserves the intimacy of a block world while
  the population creates strategy-scale consequences.

**Watch:**

- [Official overview](https://www.colonysurvival.nl/)
- [Recent new-colony gameplay search](https://www.youtube.com/results?search_query=Colony+Survival+new+colony+gameplay+2026)
- [Large colony and production tour search](https://www.youtube.com/results?search_query=Colony+Survival+huge+colony+tour+automation+2026)

**Watch for:** whether colonists feel like inhabitants or production units;
how pathing and job UX scale; how much building is aesthetic versus optimized
for defense; whether daily attacks create drama or repetitive maintenance.

**Mclone question:** could a smaller number of persistent residents make a
homestead feel alive and useful before Mclone attempts colony-simulator scale?

### 7 Days To Die

**Why study it:** 7 Days to Die gives construction an unusually clear purpose:
prepare a destructible base for recurring, escalating assault.

**Impressive or distinctive aspects:**

- A moldable world and destructible buildings allow scavenging, tunneling,
  fortification, demolition, and improvised routes.
- The horde cadence turns time into a plan-build-test-repair loop.
- Traps, electricity, automated doors, turrets, gadgets, and defensive
  positions make bases functional machines.
- Looting authored places of interest complements procedural terrain and gives
  exploration specific risk/reward spaces.
- Skills, recipes, vehicles, weapons, co-op, and difficulty customization
  support long campaigns.
- Structural failure and enemy pathing make architecture an adversarial
  design problem.

**Watch:**

- [Official site](https://7daystodie.com/)
- [Current Steam feature overview](https://store.steampowered.com/app/251570/7Daysto_Die/)
- [Recent day-one gameplay search](https://www.youtube.com/results?search_query=7+Days+to+Die+day+1+gameplay+2026)
- [Blood Moon base-design gameplay search](https://www.youtube.com/results?search_query=7+Days+to+Die+Blood+Moon+base+design+gameplay+2026)

**Watch for:** how the deadline changes ordinary gathering; what makes a base
failure understandable; where players exploit AI rather than build plausible
fortifications; whether recurring destruction strengthens attachment or
discourages expressive building.

**Mclone question:** what gentler recurring pressures could make a farm,
workshop, wall, road, light, or shelter matter without turning every home into
a horde exploit?

### Lay Of The Land

**Why study it:** Lay of the Land is a current reference for using a voxel
world as a physically simulated fantasy adventure rather than exposing a
Minecraft-like cube grid as the entire visual identity.

**Impressive or distinctive aspects:**

- Layered world simulations shape naturalistic terrain, watercourses, roads,
  and locations.
- Destruction, fire, liquids, gravity, weather, and material interaction can
  become combat or traversal tools.
- Voxel building coexists with sculpted-looking terrain and action-RPG
  presentation.
- Environmental systems create the possibility of memorable emergent events
  rather than only permanent block edits.
- The product foregrounds explore/fight/loot/build choices rather than strict
  survival simulation.

**Watch:**

- [Current Steam overview](https://store.steampowered.com/app/2776090/Lay_of_the_Land/)
- [Developer YouTube channel](https://www.youtube.com/@tooley1998)
- [Recent unedited gameplay search](https://www.youtube.com/results?search_query=Lay+of+the+Land+unedited+gameplay+2026)
- [Physics, building, and destruction search](https://www.youtube.com/results?search_query=Lay+of+the+Land+physics+building+destruction)

**Watch for:** whether simulation produces useful choices or spectacle; how
terrain remains readable when it is not a clean block grid; performance under
chain reactions; whether player buildings participate in the same physical
rules.

**Mclone question:** which material reactions—falling trees, fire, water,
moving voxel assemblies, erosion-like presentation—would create the most
player stories per unit of engine complexity?

### Luanti, VoxeLibre, And Mineclonia

**Why study them:** Luanti is not one game. It is a free voxel engine and
content platform whose games show how much Minecraft-like play can be supplied
through open code, Lua modification, an in-client content browser, and
automatic server content acquisition.

**Impressive or distinctive aspects:**

- One engine supports many games, mods, world generators, and multiplayer
  servers across desktop and Android.
- ContentDB makes discovering and installing games or mods part of the client.
- Server-provided content reduces manual setup when joining a different
  experience.
- VoxeLibre, formerly MineClone2, offers a cohesive open survival sandbox
  rather than asking players to assemble an engine themselves.
- Mineclonia is a separate Minecraft-inspired Luanti game and a useful study
  in how forks diverge when maintainers prioritize different compatibility,
  performance, and gameplay goals.
- The ecosystem establishes that "free, open, moddable, multiplayer voxel
  engine" is not by itself a premium product proposition.

**Watch:**

- [Official Luanti overview](https://www.luanti.org/en/)
- [Luanti player documentation](https://docs.luanti.org/for-players/getting-started/)
- [VoxeLibre on ContentDB](https://content.luanti.org/packages/Wuzzy/mineclone2/)
- [Mineclonia on ContentDB](https://content.luanti.org/packages/ryvnf/mineclonia/)
- [Recent VoxeLibre gameplay search](https://www.youtube.com/results?search_query=VoxeLibre+gameplay+2026)
- [Recent Mineclonia gameplay search](https://www.youtube.com/results?search_query=Mineclonia+Luanti+gameplay+2026)

**Watch for:** install-to-play friction; baseline polish before mods; how
content discovery communicates "engine" versus "game"; server join behavior;
where fragmentation helps experimentation and where it harms identity.

**Mclone question:** what cohesive authored experience, onboarding,
presentation, cross-device continuity, and support justify a premium Mclone
when open voxel capability is already abundant?

### Survivalcraft 2

**Why study it:** Survivalcraft is an important reminder that mobile-first,
paid, compact voxel survival has existed independently of Minecraft for a long
time.

**Impressive or distinctive aspects:**

- Touch-first roots and a small installation footprint focus attention on the
  core survival loop.
- Seasons, weather, temperature, clothing, shelter, traps, explosives, and
  more than thirty real-world animals create a naturalistic survival flavor.
- Riding, herding, predators, and animal resources make fauna mechanically
  central.
- Community-content sharing gives a small standalone product a creator loop.
- Furniture construction lets players derive smaller decorative forms from
  block materials.

**Watch:**

- [Official developer history and updates](https://kaalus.wordpress.com/)
- [Current iOS product description](https://apps.apple.com/app/survivalcraft-2/id1185580782)
- [Recent survival gameplay search](https://www.youtube.com/results?search_query=Survivalcraft+2+survival+gameplay+2026)
- [Furniture and advanced world tour search](https://www.youtube.com/results?search_query=Survivalcraft+2+furniture+advanced+world+tour)

**Watch for:** touch ergonomics; information density on a small display;
animal behavior; what the product omits; how a compact premium game creates
depth without a live-service content stream.

**Mclone question:** can the flat Android client feel intentionally designed
for mobile rather than merely proving that the shared engine runs there?

### Discovery 2, cyubeVR, And QuestCraft

**Why study them:** Together these products cover three different XR answers:
a store-native standalone block builder with mixed-reality tabletop play; a
high-fidelity PC-VR sandbox built around hand-scale crafting; and an
unofficial route to actual Minecraft Java on Quest.

**Impressive or distinctive aspects:**

- Discovery 2 combines embodied building with a movable, scalable tabletop
  world that makes overview interaction and mixed reality part of the fantasy.
- cyubeVR emphasizes visual atmosphere, physical crafting gestures, hand
  presence, building reach, and VR-native presentation.
- QuestCraft preserves Minecraft ownership, servers, mods, and familiarity
  while making Java run standalone on Quest.
- The set exposes three separate competitive bars: spatial interaction
  quality, standalone performance/store convenience, and access to the
  existing Minecraft ecosystem.

**Watch:**

- [Discovery 2 official page and trailer](https://www.altlabvr.com/discovery-2)
- [Discovery 2 gameplay and tabletop search](https://www.youtube.com/results?search_query=Discovery+2+Quest+tabletop+gameplay)
- [cyubeVR official developer channel](https://www.youtube.com/@StonebrickStudios)
- [cyubeVR “The Real Minecraft VR...” overview](https://www.youtube.com/watch?v=PVoecRizKyk)
- [QuestCraft official site](https://questcraft.org/)
- [QuestCraft current gameplay and setup search](https://www.youtube.com/results?search_query=QuestCraft+Quest+3+gameplay+setup+2026)

**Watch for:** arm fatigue and interaction reach; inventory and UI placement;
locomotion comfort; whether physical crafting remains pleasurable after
repetition; tabletop target precision; visual compromises needed for
standalone performance; multiplayer embodiment.

**Mclone question:** can one shared semantic interaction model produce
device-native flat and XR experiences without reducing XR to gamepad verbs or
making flat play imitate physical gestures?

### Enshrouded

**Why study it:** Enshrouded is an important adjacent competitor because it
combines detailed voxel terrain and construction with the presentation,
combat, progression, and authored-world expectations of a modern cooperative
action RPG.

**Impressive or distinctive aspects:**

- Voxel terraforming and detailed construction do not require a visibly
  one-meter cube aesthetic.
- A handcrafted world, bosses, equipment, skill trees, traversal, and story
  give exploration a stronger authored cadence than a pure procedural
  sandbox.
- Up to sixteen-player co-op broadens the social offer.
- Rich lighting, materials, props, and animation make player homes feel
  visually integrated with the adventure world.
- Survival is intentionally lighter than in Vintage Story, keeping action and
  discovery central.

**Watch:**

- [Official overview and FAQ](https://enshrouded.com/en-US/faq)
- [Official gameplay channel](https://www.youtube.com/@enshroudedgame)
- [Recent first-hour gameplay search](https://www.youtube.com/results?search_query=Enshrouded+first+hour+gameplay+2026)
- [Advanced voxel-building tour search](https://www.youtube.com/results?search_query=Enshrouded+advanced+voxel+building+tour+2026)

**Watch for:** the boundary between authored landmarks and editable terrain;
how detailed building tools stay controller-usable; whether homes have
systemic purpose; what visual techniques conceal the voxel substrate.

**Mclone question:** how visually original and materially detailed must
Mclone's first-party world become before broad survival players see a product
rather than an engine or compatibility exercise?

## Wider Product Scan

These products do not all warrant the same depth, but each demonstrates a
useful design center. Review them when the corresponding question becomes
active.

### Direct And Near-Direct Voxel Sandboxes

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Creativerse](https://store.steampowered.com/app/280790/Creativerse/) | Friendly polished block adventure, recipes, gadgets, taming, farming, teleporters, and easy multiplayer | [Gameplay search](https://www.youtube.com/results?search_query=Creativerse+gameplay+world+tour+2026) |
| [Boundless](https://playboundless.com/) | Persistent connected planets, portals, settlements, specialization, trade, claims, and MMO-scale shared construction | [Official trailer](https://www.youtube.com/watch?v=o-deAhnHXZw) · [world tour search](https://www.youtube.com/results?search_query=Boundless+voxel+MMO+world+tour+2026) |
| [Trove](https://www.trionworlds.com/trove/en/) | Fast class-based voxel MMO, instanced adventure worlds, loot, clubs, and personal cornerstone building | [Gameplay search](https://www.youtube.com/results?search_query=Trove+gameplay+club+world+tour+2026) |
| [Block Story](https://blockstory.net/) | Mobile/PC block sandbox fused with quests, levels, bosses, summons, creatures, and rideable dragons | [Gameplay search](https://www.youtube.com/results?search_query=Block+Story+gameplay+2026) |
| [ClassiCube](https://www.classicube.net/) | Extremely portable open Minecraft-Classic-style creative client, including a web client and many unusual platforms | [Gameplay search](https://www.youtube.com/results?search_query=ClassiCube+web+multiplayer+gameplay) |
| [Everwind](https://www.everwind.net/) | Co-op voxel survival RPG on floating islands, with a buildable airship-base, magic, dungeons, and vertical exploration | [Gameplay search](https://www.youtube.com/results?search_query=Everwind+voxel+survival+gameplay+2026) |

### Authored Adventure And Voxel RPG

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Portal Knights](https://store.steampowered.com/app/374040/Portal_Knights/) | Accessible third-person co-op action RPG with classes, bosses, islands, crafting, and building | [Gameplay search](https://www.youtube.com/results?search_query=Portal+Knights+first+hour+gameplay) |
| [Veloren](https://www.veloren.net/) | Open-source procedural voxel action RPG with fast combat, towns, dungeons, NPCs, mounts, caves, and multiplayer | [Gameplay search](https://www.youtube.com/results?search_query=Veloren+gameplay+2026) |
| Cube World | A useful historical study in attractive procedural exploration, class combat, expectation, and the risk of a compelling visual premise without enough durable world interaction | [Gameplay retrospective search](https://www.youtube.com/results?search_query=Cube+World+gameplay+retrospective) |

### Society, NPCs, And Living Worlds

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Terasology](https://terasology.org/) | Open-source modular voxel project explicitly influenced by Minecraft, Dwarf Fortress, and Dungeon Keeper; ambitious NPC, estate, workshop, and simulation ideas | [Official teaser](https://www.youtube.com/watch?v=Wpa2aiadwE8) · [current gameplay search](https://www.youtube.com/results?search_query=Terasology+gameplay+2026) |
| [MineColonies](https://minecolonies.com/) | Minecraft mod centered on planned settlements, builder NPCs, professions, requests, progression, and colony management | [Colony tour search](https://www.youtube.com/results?search_query=MineColonies+colony+tour+2026) |
| [Wurm Online](https://www.wurmonline.com/what-is-wurm/) | Slow persistent sandbox MMO built around terrain shaping, roads, deeds, skills, player economies, settlements, carts, ships, and work with lasting public consequences | [Terraforming and village search](https://www.youtube.com/results?search_query=Wurm+Online+terraforming+village+tour+2026) |
| [Stonehearth](https://store.steampowered.com/app/253250/Stonehearth/) | Voxel colony builder focused on settlers, moods, jobs, food, shelter, defense, and player-designed towns | [Settlement gameplay search](https://www.youtube.com/results?search_query=Stonehearth+ACE+settlement+gameplay+2026) |
| Dwarf Fortress / RimWorld | Non-voxel references for autonomous inhabitants, needs, history, failure, specialization, and stories generated by interacting simulations | [Dwarf Fortress fortress tour search](https://www.youtube.com/results?search_query=Dwarf+Fortress+fortress+tour) · [RimWorld colony story search](https://www.youtube.com/results?search_query=RimWorld+colony+story+gameplay) |

### Engineering, Automation, And Functional Construction

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Factorio](https://www.factorio.com/game/content) | The clearest reference for readable logistics, escalating automation, blueprints, throughput problems, and factories that create their own next goals | [Factory tour search](https://www.youtube.com/results?search_query=Factorio+megabase+tour+blueprints) |
| [Create](https://www.createmod.net/) | Minecraft automation made spatial, animated, inspectable, and expressive through shafts, gears, belts, trains, and contraptions | [Contraption tour search](https://www.youtube.com/results?search_query=Minecraft+Create+mod+contraption+factory+tour+2026) |
| [Space Engineers](https://www.spaceengineersgame.com/) | Functional grid construction, vehicles, ships, stations, power, conveyor systems, physics, damage, and persistent deformable planets | [Survival engineering search](https://www.youtube.com/results?search_query=Space+Engineers+survival+ship+build+gameplay+2026) |
| [FortressCraft Evolved](https://store.steampowered.com/app/254200/FortressCraft_Evolved/) | Historical voxel factory reference: deep mining, power, conveyor logistics, defenses, and very large production systems | [Factory tour search](https://www.youtube.com/results?search_query=FortressCraft+Evolved+factory+tour) |
| [Satisfactory](https://www.satisfactorygame.com/) | Adjacent first-person reference for spectacular factories, traversal, landmarks, logistics, and making production architecture visually impressive | [Factory tour search](https://www.youtube.com/results?search_query=Satisfactory+factory+tour+2026) |

### Destruction And Material Simulation

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Teardown](https://www.teardowngame.com/) | Fine-grained voxel destruction, rigid debris, tools, vehicles, heist puzzles, creative mode, mods, and multiplayer | [Official trailer](https://www.youtube.com/watch?v=4AfSSvmWkxY) · [sandbox gameplay search](https://www.youtube.com/results?search_query=Teardown+sandbox+destruction+gameplay+2026) |
| Donkey Kong Bananza | Adjacent reference for using hidden voxel material to make dense authored environments joyfully destructible without looking like stacked cubes | [Gameplay search](https://www.youtube.com/results?search_query=Donkey+Kong+Bananza+destruction+gameplay) |

### Adjacent Survival And Retention References

| Product | Distinctive center | Useful viewing |
|---|---|---|
| [Terraria](https://terraria.org/) | Extraordinary content density, bosses, gear, biomes, events, secrets, building, and long progression from a small set of verbs | [Progression gameplay search](https://www.youtube.com/results?search_query=Terraria+new+character+progression+gameplay) |
| [Core Keeper](https://pugstorm.eu/) | Approachable co-op mining, bosses, farming, base decoration, skill growth, automation, and world-to-world character continuity | [First-hour search](https://www.youtube.com/results?search_query=Core+Keeper+first+hour+co-op+gameplay+2026) |
| [Valheim](https://valheim.com/) | Strong atmosphere, biome/boss progression, food preparation, voyages, co-op, structural building, and memorable expeditions | [First-biome gameplay search](https://www.youtube.com/results?search_query=Valheim+first+biome+unedited+gameplay+2026) |
| [No Man's Sky](https://www.nomanssky.com/) | Seamless exploration fantasy, procedural spectacle, base building, discovery, expeditions, vehicles, multiplayer, and sustained update cadence | [Expedition and base tour search](https://www.youtube.com/results?search_query=No+Man%27s+Sky+expedition+base+tour+2026) |

### Creator, Social, And Catalogue References

| Product or service | Distinctive center | Useful viewing |
|---|---|---|
| [Roblox](https://www.roblox.com/) | Massive cross-device identity, social graph, discovery, creator economy, user-made experiences, and low-friction joining | [Creator/discovery overview search](https://www.youtube.com/results?search_query=Roblox+creator+discovery+platform+overview+2026) |
| [Planet Minecraft](https://www.planetminecraft.com/) | Community skins, maps, builds, packs, servers, discovery, contests, and downloadable artifacts | Review category pages, ranking, previews, attribution, and handoff friction |
| [GrabCraft](https://www.grabcraft.com/) | Screenshot-led structure catalogue, layer plans, material lists, and search acquisition | Review how quickly a visitor can understand and reproduce one build |
| [Modrinth](https://modrinth.com/) / [CurseForge](https://www.curseforge.com/minecraft) | Mod and modpack discovery, dependency/version management, creators, distribution, and compatibility expectations | Follow one user from search result to a working multiplayer-compatible pack |

### Historical Lineage And Control Cases

Historical products help separate durable genre ideas from assumptions created
by modern Minecraft:

| Product | Why it still matters | Useful viewing |
|---|---|---|
| Infiniminer | A key pre-Minecraft reference for competitive class-based mining, construction, ore, and a freely destructible block field | [Gameplay/history search](https://www.youtube.com/results?search_query=Infiniminer+gameplay+history) |
| [Wurm Online](https://www.wurmonline.com/features/) | Persistent terrain modification, deeds, roads, villages, skills, tools, ships, and player economies demonstrate a slower simulation-heavy sandbox lineage | [Village showcase search](https://www.youtube.com/results?search_query=Wurm+Online+village+showcase+terraforming) |
| [Total Miner](https://store.steampowered.com/app/347600/Total_Miner/) | The Xbox indie-era branch of block sandboxes, with creative/RPG tooling and the unusually deep vertical `Dig Deep` mode | [Original launch trailer](https://www.youtube.com/watch?v=XO3TEFazvUI) |
| CastleMiner Z | Another Xbox indie-era control: familiar block building joined directly to guns, dragons, distance progression, and survival pressure | [Gameplay retrospective search](https://www.youtube.com/results?search_query=CastleMiner+Z+gameplay+retrospective) |
| [Build and Shoot / Ace of Spades Classic](https://www.buildandshoot.com/) | Destructible terrain and rapid construction used tactically inside a competitive team shooter | [Classic gameplay search](https://www.youtube.com/results?search_query=Ace+of+Spades+Classic+Build+and+Shoot+gameplay) |
| Blockland | LEGO-like construction, scripting, vehicles, minigames, and community servers show a building-to-UGC path that is not survival-led | [Server and build retrospective search](https://www.youtube.com/results?search_query=Blockland+server+build+retrospective) |
| [Stonehearth](https://store.steampowered.com/app/253250/Stonehearth/) | A voxel world used as the substrate for a warm authored colony simulation rather than first-person survival | [Town tour search](https://www.youtube.com/results?search_query=Stonehearth+town+tour+ACE) |

The historical set is especially valuable when evaluating a proposed feature
that sounds new only because it is uncommon in current Minecraft. Review the
older interaction in context, including why it did or did not support a
durable product.

## The Minecraft Mod And Tool Ecosystem Is A Competitor

A standalone game does not compete only with vanilla Minecraft. A player can
assemble a more focused Minecraft proposition without leaving the category
owner:

| Desired proposition | Minecraft route | What to study |
|---|---|---|
| Deep material survival | [TerraFirmaCraft](https://www.curseforge.com/minecraft/mc-mods/terrafirmacraft) and modpacks built around it | Geology, seasons, food, metal ages, and how a total-conversion mod teaches unfamiliar rules |
| Spatial automation | [Create](https://www.createmod.net/) | Visible motion, understandable power transfer, kinetic spectacle, trains, and machines that also decorate a world |
| Living settlement | [MineColonies](https://minecolonies.com/) | NPC requests, building placement, staged upgrades, professions, and colony-scale purpose |
| Guided construction | [Litematica](https://www.curseforge.com/minecraft/mc-mods/litematica) and building blueprints | Ghost placement, layer inspection, material verification, and survival build-along friction |
| Large-scale editing | [WorldEdit](https://enginehub.org/worldedit/) | Selection, transforms, brushes, scripts, undo, and why creators outgrow one-block-at-a-time tools |
| Vast sightlines | [Distant Horizons](https://modrinth.com/mod/distanthorizons) and Voxy | How extreme view distance changes exploration, landmarks, navigation, and performance expectations |
| PC VR | [Vivecraft](https://www.vivecraft.org/) | Retrofitted motion controls, embodiment, comfort, multiplayer compatibility, and mod interaction |
| Standalone Quest | [QuestCraft](https://questcraft.org/) | The value of actual Minecraft access versus setup, ownership, performance, support, and store friction |
| Curated total experience | Major Modrinth/CurseForge modpacks | How curation, quests, defaults, updates, and server packs turn many mods into a coherent product |
| Minigames and social servers | Hypixel and other major networks | Fast onboarding, lobbies, parties, progression, events, moderation, and repeatable session-scale goals |

This matters strategically. "Mclone has deeper survival," "Mclone has
automation," or "Mclone has colonies" is not differentiated if a familiar
Minecraft installation can add a mature version of that system. Mclone's
advantage must come from coherence, native multi-device design, lower friction,
original identity, and combinations that are difficult to assemble as mods.

## Watchlist And Periodic Recheck

### Reforj

[Reforj](https://reforj.game/about/) is an upcoming open-world sandbox from 4J
Studios, the team historically responsible for Minecraft's console editions.
Its current proposition emphasizes procedural worlds, settlements across
hostile worlds, and intuitive sculpting that transforms blocks into different
shapes. It is strategically relevant because it combines deep knowledge of
controller/console Minecraft with a new engine and an original product.

- [Official 4J Studios YouTube channel](https://www.youtube.com/@4JStudios)
- [Current Reforj gameplay search](https://www.youtube.com/results?search_query=Reforj+gameplay+4J+Studios+2026)

Recheck release state, platforms, multiplayer, world persistence, building
granularity, and creator tooling when a public milestone lands.

### Allumeria

[Allumeria](https://allumeria.com/) is a small direct voxel-sandbox RPG with a
public demo and a 2026 Early Access target. Its current pitch is "depth, not
breadth": biomes as progression chapters, dungeons and bosses, many movement
and combat tools, full-RGB lighting, block painting, and the ability to change
block shapes. It is useful both as a feature study and as a brand-distance
control because its familiar visual language makes comparison with Minecraft
immediate.

- [Current Steam page and demo](https://store.steampowered.com/app/3516590/Allumeria/)
- [Gameplay and building-tools search](https://www.youtube.com/results?search_query=Allumeria+gameplay+building+tools+2026)

Recheck the release build, multiplayer, visual identity, biome depth, and how
much the shape/paint tools improve ordinary survival construction.

### Light No Fire

[Light No Fire](https://lightnofire.com/) is Hello Games' announced fantasy
survival sandbox on a procedural planet described as Earth-sized. Its stated
pillars—adventure, building, survival, exploration together, and RPG depth—make
it an important expectation benchmark even though its world is not presented
as a Minecraft-like block grid.

- [Official trailer via the product site](https://lightnofire.com/)
- [Current gameplay/news search](https://www.youtube.com/results?search_query=Light+No+Fire+official+gameplay+2026)

Recheck actual building freedom, persistence, multiplayer topology, traversal,
settlements, and whether planetary scale produces meaningful discovery rather
than undifferentiated area.

### KYORA

[KYORA](https://store.steampowered.com/app/3337850/KYORA/) is a Pugstorm
multiplayer sandbox in which every pixel can be shaped, mined, built, or
destroyed. It is a useful adjacent watch because it explores finer material
granularity, authored pixel art, physical reactions, combat, and cooperative
progression without inheriting the one-meter-block convention.

- [Developer overview](https://pugstorm.eu/)
- [Current gameplay search](https://www.youtube.com/results?search_query=KYORA+Pugstorm+gameplay+2026)

Recheck release state, simulation density, building readability, multiplayer,
and how fine-grained editing avoids becoming tedious.

### New And Small Voxel Sandboxes

Periodically search current Steam, Meta Horizon Store, itch.io, SideQuest, and
Luanti ContentDB releases. Small products can establish one excellent mechanic
without becoming broad commercial competitors. Prioritize anything showing:

- store-native standalone XR survival rather than only creative building;
- a full browser survival client with persistent multiplayer;
- meaningful simulation of ecology, geology, fluids, fire, structures, or
  settlements;
- unusually strong touch or controller building tools;
- live overview/tabletop play;
- cross-device continuation of one world;
- semantic blueprints, guided construction, or AI-assisted building; or
- a visual identity that escapes generic pixel-textured cubes.

Do not fill the main comparison with low-effort mobile reskins. Add a product
when it owns an instructive mechanic, audience, distribution route, or failure
mode.

## Mclone Inspiration Ledger

This is a question ledger, not a commitment list:

| Observed strength | Reference | Mclone question or possible experiment | Existing owner |
|---|---|---|---|
| Manual transformation makes materials memorable | Vintage Story | Which one or two crafts deserve spatial or staged interaction rather than a generic recipe click? | Future shared crafting/inventory contract |
| Seasons turn a homestead into a plan | Vintage Story | Would one readable first-party seasonal loop improve attachment before adding broad survival meters? | Original Overworld / future simulation topic |
| Characters validate and help complete builds | Dragon Quest Builders 2 | Can residents react to rooms, plans, farms, and settlement milestones? | Entity, settlement, and structure topics |
| Specialization creates cooperation | Eco | Can optional roles make mixed-device friends complementary without blocking solo play? | Multiplayer/gameplay product design |
| Buildings become production systems | Create, Factorio, Colony Survival | What is the smallest inspectable machine or workshop loop worth building? | Future shared automation contract |
| Recurring pressure gives walls a purpose | 7 Days to Die, Colony Survival | What pressure creates preparation and stories without routine destruction of expressive homes? | Future gameplay/actor design |
| Material reaction creates emergent stories | Lay of the Land, Teardown | Which bounded fire, tree, water, collapse, or moving-assembly interaction has the best cost-to-story ratio? | Liquids, falling-tree, renderer, and simulation topics |
| A miniature world improves building and comfort | Discovery 2 | Can one overview mode remain useful on flat, touch, and XR while preserving ordinary authority? | [`tabletop-overview-mode.md`](tabletop-overview-mode.md) |
| Rich creator tools outperform block-by-block editing | Hytale, WorldEdit | Which selection, transform, prefab, and semantic edit operations belong in-game? | Structure Lab and future creative-mode contract |
| A complete open stack creates trust, preservation, and unlimited total conversions | Luanti, Terasology, Veloren; contrast Vintage Story and Hytale | Can all first-party clients, server, tools, protocols, gameplay data, and original content be legally rebuilt and forked while official releases remain the easiest trusted product? | Product licensing/governance decision; then release, asset, server, and mod contracts |
| Open source and good mods solve different jobs | Factorio, Luanti, Vintage Story, Teardown, cyubeVR | What stable package API, discovery flow, sandbox, dependency model, and server-delivery contract lets an ordinary player benefit without forking the engine? | Future shared content/mod platform contract |
| A build page becomes playable immediately | No exact peer in the reviewed set | Can a catalogue page enter the actual web client quickly enough to outperform video or screenshots? | [`structure-catalogue-product.md`](structure-catalogue-product.md) |
| Friends can return without the world owner | LEGO Fortnite Odyssey, dedicated servers | Which host topology gives small groups durable convenience without mandatory central hosting? | Multiplayer and browser-hosted session topics |
| Detailed voxel construction can escape cube aesthetics | Enshrouded, Reforj | Which first-party blocks, trims, compiled structures, or sculpting tools create identity without losing block readability? | Asset, mesh, structure, and original-world topics |
| Huge sightlines change world meaning | Distant Horizons, Voxy | Which landmarks, navigation, and terrain composition make distant terrain a product feature rather than only optimization? | retired Far LOD evidence and procedural-terrain/original Overworld topics |

## Review Template

Use this for a focused play/video review:

```markdown
### Product — version/date

Sources and footage:
- official:
- short product video:
- unedited early play:
- advanced/endgame/build/server tour:

Core fantasy:
- one sentence:
- first fifteen-second proof:

Loop:
- first thirty minutes:
- ten-hour arc:
- long-term return reason:

Impressive:
- interaction:
- world/simulation:
- presentation:
- social/creator:

Friction or tradeoffs:
- onboarding/UI:
- repetition:
- performance/platform:
- multiplayer/content:

Open/source/mod posture:
- client, server, engine, and tool licenses:
- first-party content and asset licenses:
- buildability, self-hosting, protocols, and proprietary dependencies:
- data, code, script, and creator-tool extension surfaces:
- discovery, install, dependencies, updates, and server join:
- compatibility, sandbox, permissions, attribution, and governance:

Mclone:
- transfer directly:
- adapt rather than copy:
- deliberately reject:
- new question:
- relevant topic owner:
```

When review-volume or recurring-complaint work begins, use a reproducible
sample: record storefront, language, date range, game version, total reviews
available, selection rule, and representative positive and negative themes.
Do not infer general sentiment from a few videos or highly ranked reviews.

## Recommended Familiarization Sequence

For the fastest broad understanding, review in this order:

1. **Vintage Story:** the strongest direct lesson in coherent survival depth.
2. **Hytale:** the strongest voxel-RPG and creator-tool ambition.
3. **Dragon Quest Builders 2:** story, characters, teaching, and celebration.
4. **Eco:** systemic consequences, specialization, economy, and governance.
5. **Colony Survival:** NPC labor and functional settlements.
6. **7 Days to Die:** recurring pressure and adversarial construction.
7. **LEGO Fortnite Odyssey:** social access, cross-play, modes, and inhabited
   villages.
8. **Lay of the Land and Teardown:** physical material and destruction.
9. **Discovery 2 and cyubeVR:** tabletop and VR-native interaction.
10. **Luanti/VoxeLibre/Mineclonia:** the free/open/moddable baseline; inspect
    ContentDB, package licenses, a server join, and one simple Lua mod as well
    as gameplay.
11. **Survivalcraft 2:** mobile-first constraint and compact survival.
12. **Enshrouded:** modern visual, building, and action-RPG expectations.
13. **Minecraft mod routes:** Create, TerraFirmaCraft, MineColonies,
    Litematica, WorldEdit, Distant Horizons, Vivecraft, and a curated modpack.
14. **Open/mod control cases:** build Terasology, Veloren, or ClassiCube from
    source; then install one Factorio, Teardown, or cyubeVR mod to compare
    whole-project freedom with a polished package workflow.

This sequence moves from direct gameplay competitors to systems specialists,
then returns to the ecosystem that can already combine many of those
specialists inside Minecraft.

## Evidence To Gather Next

1. Complete one review template for each product in the recommended sequence.
2. Capture dated platform, price, demo/trial, controller, cross-play, server,
   modding, and update-cadence facts in a separate market appendix.
3. Complete a license/source audit that separately records client, server,
   tools, protocols, first-party gameplay code/data, assets, build
   reproducibility, mod API, package distribution, governance, and trademark
   policy. Do not reduce this to one “open source: yes/no” column.
4. Sample recurring player praise and complaints using a reproducible review
   method.
5. Record a screenshot or timestamped clip for every claimed signature
   interaction; prose alone makes visual comparisons too abstract.
6. Build a capability × product matrix only after the deep reviews define
   terms such as "living world," "native XR," "creator tools," and
   "cross-platform" precisely.
7. Convert inspiration into the smallest relevant topic question. Do not open
   a tactical merely because another product has an impressive feature.
8. Reconcile the resulting commercial lessons back into
   [`distribution-go-to-market.md`](distribution-go-to-market.md) without
   moving this design-oriented catalogue into that document.

## Related Documents

- [`scripting-and-mod-platform.md`](scripting-and-mod-platform.md) — accepted
  package tiers, portable runtime, capability sandbox, gameplay profiles,
  registry, mod browser, and open-source extension direction.
- [`distribution-go-to-market.md`](distribution-go-to-market.md) — commercial
  channels, competitive distribution openings, positioning, and launch
  sequence.
- [`structure-catalogue-product.md`](structure-catalogue-product.md) — the
  walkable AI-authored structure catalogue and acquisition proposition.
- [`tabletop-overview-mode.md`](tabletop-overview-mode.md) — shared flat,
  touch, and XR active-world overview research.
- [`embedded-worlds.md`](embedded-worlds.md) — live world previews,
  destinations, dioramas, and warm transitions.
- [`game-title-and-brand-identity.md`](game-title-and-brand-identity.md) —
  original public identity and avoidance of generic voxel/Minecraft trade
  dress.
- [`mclone-overworld-generation.md`](mclone-overworld-generation.md) — the
  original first-party world's terrain and visual direction.
- [`local-couch-multiplayer.md`](local-couch-multiplayer.md) — mixed-role and
  mixed-presentation local play.
- [`controller-input.md`](controller-input.md) and
  [`touchscreen-input.md`](touchscreen-input.md) — native interaction across
  controller, touch, and tracked-controller classes.
