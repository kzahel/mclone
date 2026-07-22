# Distribution And Go-To-Market Strategy

Topic: `distribution-go-to-market`

Status: **current product direction recorded 2026-07-22; commercial model,
pricing, launch order, and storefront approvals remain open.** Steam should be
the preferred PC purchase and discovery surface, not a channel from which
Mclone steers customers away. A first-party direct edition should provide a
real no-Steam option. The leading commercial candidate is a free trial or demo
plus paid ownership in each channel, while a complete free direct edition with
paid supporter/store editions remains an explicit alternative.

Last reconciled: **2026-07-22**.

External market and policy facts in this document are a dated snapshot, not a
substitute for reviewing the executed Steam, Meta, payment-provider, and other
distribution agreements before launch.

## Scope

This topic owns the current commercial distribution and go-to-market direction
for a public Mclone release:

- the role of Steam, direct desktop, Meta Horizon Store / Quest, web, flat
  Android, and dedicated-server channels;
- free trial, paid game, full-free-direct, supporter, and entitlement options;
- channel priority without artificial exclusivity or steering;
- target audiences, positioning, launch sequence, acquisition loops, and
  success measures;
- the competitive landscape, especially the Minecraft access gaps on Steam
  Deck and standalone Quest; and
- commercial, policy, brand, and product risks that must be settled before a
  public offer is announced.

It does **not** own:

- launcher, updater, artifact, signing, transactional install, or update-delta
  mechanics; those live in
  [`release-distribution-and-updates.md`](release-distribution-and-updates.md);
- current platform implementation or validation status; that lives in
  [`../platforms.md`](../platforms.md) and
  [`platform-parity.md`](platform-parity.md);
- public asset provenance and the no-Minecraft-content release boundary; that
  lives in [`asset-pack-profiles.md`](asset-pack-profiles.md);
- the structure catalogue's separate product and acquisition-loop proposal;
  that lives in
  [`structure-catalogue-product.md`](structure-catalogue-product.md); or
- legal, tax, accounting, age-rating, privacy, or storefront approval itself.

## Strategic Thesis

Mclone should be sold as an **original, cross-platform voxel survival sandbox
that is native to both ordinary screens and spatial play**. Minecraft parity is
an internal development and correctness technique, not the public identity.

The strongest product advantages are the combination rather than any one
feature:

- one shared game and world model across desktop, Steam Deck, web, flat
  Android, desktop OpenXR, and standalone Quest;
- first-class flat, ordinary-controller, touch, and tracked-controller
  experiences instead of a flat game wrapped after the fact;
- local integrated play, dedicated remote play, and a path to cross-platform
  sessions;
- a normal store install where competing Minecraft access is awkward or
  absent; and
- a supported no-store edition for players who deliberately avoid Steam or
  Meta distribution.

The internal competitive lens may be “Minecraft is awkward here,” but the
external promise must describe what Mclone positively is. Do not market it as
“Minecraft 1.17.1,” “Minecraft compatible,” or a substitute using Minecraft
trade dress. Public screenshots, trailers, packages, and demos must contain
only releasable first-party content.

## Current Channel Direction

Steam is the preferred PC storefront. “Preferred” means the website, launch
campaign, and in-product Steam experience should make buying and playing on
Steam easy. The direct edition exists to include people who do not want Steam;
it is not a funnel designed to extract Steam customers into an external
checkout.

Quest is a distinct platform opportunity, not merely another build artifact.
The absence of an official native Minecraft product creates a stronger product
gap than on Steam Deck, but the smaller market, store review, performance,
comfort, and youth-safety requirements make it a high-quality bar rather than
an automatic win.

Web is primarily the lowest-friction acquisition and trial surface at first.
It can become a complete product surface when persistence, performance,
networking, and browser deployment are ready for that promise, but the GTM plan
must not imply that readiness prematurely.

The direct desktop edition is a durable accessibility and independence lane.
It should be easy to find on the first-party website, receive the same gameplay
and content, and use the managed launcher/update architecture. It need not be
the default call to action.

## Proposed Channel Portfolio

| Channel | Product role | Commercial candidate | Installation and update posture |
|---|---|---|---|
| Steam desktop flat/OpenXR | Preferred PC purchase, discovery, community, and Steam Deck surface | Paid base game plus a proper Steam demo; optional supporter DLC only if it has honest independent value | Steam owns the payload. Avoid an extra launcher in the normal Steam path, especially on Deck. |
| First-party desktop | Supported no-Steam choice for Windows, macOS, and Linux | Free direct trial followed by a first-party one-time purchase is the leading candidate | First-party launcher owns install, repair, channels, and updates. |
| Web/WASM | Instant try-before-install funnel, share links, catalogue/world previews, and possibly a later full client | Free bounded demo initially; full free or entitled play remains open | Web deployment owns activation and caching. |
| Meta Horizon Store / Quest | High-differentiation standalone-XR purchase surface | Paid premium app, with a platform-native trial/demo only if the current store program supports the intended shape | Meta owns store installation and updates. |
| Direct Quest APK | Enthusiast, tester, archival, and no-store lane | Signed trial or entitled direct build; a complete free build remains an option | Android package installation and user confirmation own upgrades. |
| Flat Android store/direct | Broad mobile reach after touch UX and product readiness | Undecided; avoid assuming the Quest offer transfers cleanly | Store or Android package owner controls executable updates. |
| Dedicated server | Community retention, multiplayer, and creator infrastructure | Prefer free server software; hosted service economics are a separate later decision | Operators, containers, or packages own update cadence. |

No distribution flavor should gain better gameplay, world formats, rendering,
or content merely because its store takes payment. Store achievements, cloud
saves, presence, entitlement adapters, and package identity may legitimately
differ. Cross-play and behavioral parity do not imply cross-buy.

## Commercial Models Under Consideration

| Model | Shape | Benefits | Main problems | Current posture |
|---|---|---|---|---|
| Channel-native paid game with free demos | Steam, direct, and Quest each sell the full game; Steam/browser/direct trials are free | Conventional customer expectation, clear value, straightforward store ownership, and no appearance that one buyer subsidized an identical free product | Requires direct checkout, entitlement, tax/refund support, and a genuinely persuasive demo | **Leading candidate** |
| Full game free direct; paid store editions as support/convenience | First-party desktop/APK/web remain complete and free; Steam/Quest purchases are voluntary support with store convenience | Maximizes access and preserves a strong no-gate principle | Paid store value can be confusing, conversion may be weak, and store policy/price treatment needs written confirmation | Preserve as an explicit alternative, not the current default |
| Free base everywhere plus supporter products | Same free game on all channels, with soundtrack, cosmetic acknowledgement, supporter badge, or other add-ons | Clean gameplay parity and broad reach | Creates ongoing monetization/content operations and can weaken a premium-game launch | Fallback if premium conversion is rejected by audience evidence |
| Paid store game with a free direct build deliberately kept obscure | Store is marketed; direct is technically available but hidden | May reduce immediate channel conflict | Violates the goal of supporting no-store players and creates distrust if discovered | Rejected |
| Free Steam shell unlocked by external payment | Steam app is free but blocks continued play until a website payment/license | Preserves one account entitlement in theory | Steam publicly says it does not support “paywall” games and directs developers to use demos; in-game Steam purchases have Steam Wallet requirements | Rejected absent explicit written exception |

The leading offer is therefore:

1. Buy on Steam as the primary PC call to action.
2. Use a Steam demo to try the Steam edition.
3. Offer “buy direct / no Steam required” as a clear secondary website choice.
4. Let the browser or direct launcher provide a corresponding free trial.
5. Sell the Quest build through the Meta Horizon Store, with a signed direct
   APK path for testers and users who knowingly choose sideloading.
6. Keep the game, public assets, worlds, networking, and update compatibility
   behaviorally equivalent across those offers.

Steam's current pricing documentation says that a free app may not stop at a
paywall and recommends a separate demo for try-before-buy. Its in-game purchase
documentation also says Steam customers must use Steam Wallet for purchases
initiated in the Steam build. That makes a conventional paid Steam app plus
Steam demo materially cleaner than a lightweight Steam client that asks for a
first-party license:

- [Steam pricing and business-model guidance](https://partner.steamgames.com/doc/store/pricing)
- [Steam demos](https://partner.steamgames.com/doc/store/application/demos)
- [Steam in-game purchase requirements](https://partner.steamgames.com/doc/features/microtransactions)

Steam's public key rules do not conclusively answer every non-key direct-sale
configuration. Before announcing a full game that is free direct and paid on
Steam, or promising that one channel's purchase unlocks another channel,
obtain written guidance for the exact SKU and entitlement design. Apply the
same discipline to Meta: public web documentation is not a replacement for the
current developer agreement and app-review response.

## Entitlement And Account Direction

Entitlement should follow the purchase owner first:

- a Steam customer owns the Steam edition through Steam;
- a Meta customer owns the Quest Store edition through Meta;
- a direct customer owns the direct edition through a first-party entitlement;
  and
- a demo user needs no paid entitlement.

An Mclone identity may later connect cross-platform friends, servers, cloud
saves, moderation, or linked purchases, but it should not be required for
offline solo play merely because an account service exists. On Steam, prefer
automatic or low-friction SteamID linking over a mandatory manual registration
wall. Steam's own guidance warns that separate account creation is a major
audience hurdle.

Cross-play, cross-save, and cross-buy are separate promises:

- **Cross-play** means compatible clients can join the same sessions.
- **Cross-save** means explicitly linked users can move or synchronize worlds
  safely.
- **Cross-buy** means a purchase in one channel grants another channel's
  executable entitlement.

Only cross-play is part of the core product direction today. Do not imply
cross-buy until storefront policy, fraud/refund behavior, revocation, and
account recovery have been designed and approved.

For direct payments, “self-hosted payments” should mean first-party ownership
of the offer, customer relationship, and entitlement—not raw card handling.
Evaluate a payment processor or merchant-of-record arrangement for regional
tax, invoices, refunds, fraud, and chargebacks. The game and launcher should
receive a narrow signed entitlement or offline lease, never payment
credentials.

The direct edition should remain reasonably offline-capable. A one-time online
activation with a durable signed receipt is preferable to requiring a live
license server on every launch. Exact household/device limits and recovery are
open commercial decisions.

## Trial And Demo Shape

The trial should demonstrate the actual loop without feeling like a crippled
full build. Prefer a bounded experience over a countdown timer:

- one polished first-party seed, settlement, or guided opening;
- enough gathering, crafting, building, exploration, combat, and persistence
  to establish the product promise;
- an honest boundary such as selected worlds, progression, or remote-server
  access rather than an unexpected mid-session lock;
- world/save continuation after purchase where the channel permits it; and
- identical fundamental controls, rendering, comfort, and performance to the
  paid edition.

The Steam demo should be a real Steam demo App ID, not the paid app in an
externally licensed state. The website can offer instant web play and/or a
direct-launcher trial. The Quest trial shape must be chosen from the platform
program actually available at submission time rather than assumed now.

## Competitive Landscape Snapshot

This snapshot was checked on 2026-07-22. It should be rechecked before pricing,
store-page copy, or launch sequencing is approved.

| Product or route | Current strength | Gap or lesson for Mclone |
|---|---|---|
| Minecraft Java/Bedrock on ordinary PC | The category owner, with immense content, brand, creator, server, and mod ecosystems. Minecraft's normal PC route is its own launcher or Microsoft Store rather than a main-game Steam listing. | Do not compete on “same game, fewer features.” Compete on native cross-platform/XR product shape, original identity, low-friction target support, and focused quality. |
| Minecraft Java on Steam Deck | Java runs on Linux, but the common Deck experience still uses a launcher/non-Steam setup and community controller integration; [Prism Launcher publishes a dedicated Steam Deck package](https://prismlauncher.org/download/steam-deck/). | The opening is convenience: native Steam installation, controller-complete UI, correct glyphs, no desktop-mode setup, and an eventual Deck Verified result. It is not an absence-of-software opening. |
| Official Minecraft VR/MR | Mojang announced that VR/MR support would end after March 2025, and there is no normal standalone Quest Minecraft release. | Standalone Quest has the clearest category vacancy. A store-native, performant survival sandbox can solve a real distribution problem rather than merely improve convenience. |
| [QuestCraft](https://questcraft.org/) | Runs Minecraft Java standalone on Quest using Vivecraft/Pojlib and preserves actual Minecraft ownership, mods, and multiplayer value. | It is distributed through SideQuest/GitHub and depends on Java ownership and unofficial integration. Mclone can win on ordinary store installation, first-party support, onboarding, performance consistency, and no Java/Microsoft dependency. |
| [Discovery 2](https://www.altlabvr.com/discovery-2) | A real Quest block-building competitor, with creative building, sharing, electrical systems, and a widening 2026 footprint on [Steam](https://store.steampowered.com/app/4486860/Discovery_2/) and [Nintendo Switch](https://www.nintendo.com/en-gb/Games/Nintendo-Switch-download-software/Discovery-2-3070566.html). | The Quest niche is not empty. Mclone needs survival depth, living worlds, multiplayer, cross-device continuity, and a stronger original identity—not merely block placement in VR. |
| [cyubeVR](https://store.steampowered.com/app/619500/cyubeVR/) | Established premium PC-VR voxel sandbox emphasizing visual quality and hands-on VR crafting/building interactions. | It is the quality benchmark for VR-native interaction and presentation, even where it is not a standalone-Quest answer. Mclone cannot treat tracked controls as a gamepad remap. |
| [Hytale](https://hytale.com/news/2026/1/2026-01-13-hytale-is-finally-here/) | High-mindshare voxel RPG/sandbox, released in paid PC Early Access through its own account, store, and launcher in January 2026. | Direct paid distribution can support a large launch, but Hytale raises PC expectations for original art direction, creator tools, servers, and visible momentum. Mclone still benefits from Steam and XR reach that Hytale's current direct PC offer does not provide. |
| [Vintage Story](https://vintagestory.info/en/play/) | Deep survival/crafting proposition with paid direct distribution and no Steam release. | Demonstrates durable demand for a focused direct voxel product, while also illustrating the discovery and customer-preference cost of staying off Steam. |
| [Luanti](https://www.luanti.org/en/) | Free, open-source voxel creation engine/platform across desktop and Android with mods, multiple games, and multiplayer. | Free access and extensibility already exist. Mclone's premium case must be a cohesive authored game, presentation, onboarding, and multi-target experience rather than engine capability alone. |

Minecraft's own 1.21.40 changelog records the VR/MR sunset:
[Minecraft Bedrock 1.21.40](https://www.minecraft.net/en-us/article/minecraft-1-21-40-bedrock-changelog).
Its current launcher documentation lists Windows, macOS, and Linux support:
[Minecraft Launcher help](https://help.minecraft.net/hc/en-us/articles/23907917790093-Download-and-Install-the-Minecraft-Launcher).

Meta reported at GDC 2026 that Quest usage reached an all-time high in 2025 and
that more than 100 titles generated at least $1 million in gross revenue. That
is evidence that the platform can support premium software, not a revenue
forecast for Mclone:
[Meta developer recap](https://www.linkedin.com/posts/chris-pruett-0253264_yesterday-i-spoke-at-gdc-about-the-state-activity-7437932977675902977-hXrg).

## Market Implications

### Steam and Steam Deck

Steam should provide distribution, trust, wishlists, reviews, updates,
community, and a controller-native handheld experience. Target a native Linux
build and the complete Steam Deck compatibility checklist rather than merely
“runs on Linux.” Valve's current checklist requires controller access to all
content, matching glyphs, controller-compatible text input, and
controller-navigable launchers; it strongly recommends avoiding a required
launcher:
[Steam Deck compatibility review](https://partner.steamgames.com/doc/steamhardware/compat).

Consequences for the product:

- the Steam build should normally enter the game directly;
- title, world creation, settings, account linking, text entry, and recovery
  must all be usable without desktop mode, a touchscreen fallback, or a
  keyboard;
- Steam Input can enhance the experience later, but ordinary controller
  completeness is a launch requirement; and
- “Deck Verified” should be treated as a GTM deliverable backed by Valve's
  actual review, not self-awarded copy.

### Standalone Quest

Quest's opportunity is sharper: remove every workaround between owning the
headset and entering a persistent voxel world. The product proof is not “the
APK launches.” It is:

- store install and automatic platform updates;
- no PC tether, Java runtime, developer mode, or unrelated game entitlement;
- stable local world persistence and reliable multiplayer;
- comfortable locomotion and controller-complete menus;
- interactions designed for tracked controllers rather than only mapped from
  flat input; and
- a sustained performance floor under real world streaming and gameplay.

Meta's current optimization guidance says Quest apps must sustain at least
72 FPS for the Virtual Reality Check requirements:
[Meta Quest performance guidance](https://developers.meta.com/horizon/documentation/unreal/po-perf-opt-mobile/).
The existing Mclone Quest performance records and validation policy determine
whether the product is ready to make that promise.

Quest should be treated as a potential beachhead for differentiation, not
necessarily the largest revenue channel. Steam can remain the preferred PC
channel while Quest supplies the clearest “why this game?” story.

### Direct And Web

The website should not be a generic brochure. It can be a working funnel:

```text
search / creator video / shared build / store discovery
                         |
                         v
                first-party landing page
                 +-- play web demo
                 +-- download direct trial
                 +-- wishlist / buy on Steam (primary PC purchase)
                 +-- buy direct, no Steam required
                 +-- view Quest availability
                         |
                         v
               create first persistent world
                         |
                         v
              return, purchase, invite, share
```

The structure catalogue can feed this loop through searchable builds,
walk-through previews, deterministic guides, and “open in Mclone” experiences,
but catalogue traffic should not be counted as game demand until users cross
into a playable experience.

## Target Audiences

Initial messaging should distinguish audiences rather than collapse them into
“people who like Minecraft”:

1. **Steam survival/building players** who want a polished original sandbox,
   easy multiplayer, and long-lived worlds.
2. **Steam Deck players** who value instant controller-complete play without
   installing a community launcher or control mod.
3. **Standalone Quest players** who currently choose between sideloaded
   Minecraft Java and much narrower store-native block builders.
4. **No-store and Linux-first players** who deliberately value direct
   ownership, offline capability, dedicated servers, and supported downloads.
5. **Builders and creators** entering through structures, seeds, screenshots,
   videos, build guides, and eventually shareable worlds.
6. **Mixed-device groups** for whom one person can play flat while another
   joins from XR, mobile, or web.

The launch campaign need not address all six equally. Steam survival/building
players are the broad commercial audience; standalone Quest is the sharpest
differentiation wedge; direct/Linux users are an important trust and advocacy
audience; web and catalogue users form the acquisition surface.

## Positioning And Message Hierarchy

Provisional message hierarchy:

1. **A living voxel survival world built for screen and headset.**
2. **Play the same game flat or immersive, locally or with friends.**
3. **Native on the devices where voxel sandboxes are currently awkward.**
4. **Buy on your preferred store, or use the supported direct edition.**

Proof in trailers and demos should lead with player-visible facts:

- start or resume a beautiful first-party world quickly;
- gather, craft, build, explore, fight, and encounter living actors;
- move the same world or session between meaningful device classes;
- show genuine standalone Quest footage and genuine Deck/controller footage;
- show multiplayer composition rather than only empty terrain flyovers; and
- identify Early Access or unfinished systems plainly.

Avoid leading with engine implementation, Rust, seed parity, renderer
architecture, generated asset pipelines, or update diffing. Those are valuable
developer-story material after the product fantasy is clear.

## Launch Sequence

### Stage 0 — product proof and commercial groundwork

- Settle the public name, trademark search, first-party visual identity,
  ratings strategy, privacy surface, and releasable asset audit.
- Define the paid-game boundary and one trial contract shared by Steam, direct,
  and web where platform rules permit.
- Establish support, refund, tax, payment, entitlement, crash-reporting, and
  account-recovery ownership.
- Capture representative real-device footage only after first-session UX,
  persistence, and performance support the advertised promise.

### Stage 1 — audience and wishlist foundation

- Publish the Steam Coming Soon page once the capsule, trailer, screenshots,
  tags, and description represent a product that can survive long-term public
  comparison.
- Provide a focused website with Steam as the primary PC call to action and a
  visible no-Steam alternative.
- Use the public first-party web/catalogue surfaces, short gameplay clips,
  development updates, and creator outreach to test which message actually
  earns play and return behavior.
- Begin Quest playtests through the platform's current testing/release tools;
  do not use sideload success as store-readiness evidence.

Steam requires a Coming Soon page to be public for at least two weeks before
release, but the useful audience-building period is normally much longer:
[Steam release options](https://partner.steamgames.com/doc/store/types).

### Stage 2 — public demo

- Release a high-quality Steam demo associated with the paid app.
- Offer the corresponding direct and/or browser trial without steering users
  out of the Steam demo.
- Preserve demo worlds into the full product where practical.
- Measure first-world completion, return, wishlist, purchase intent, crash
  rate, and target-specific friction before declaring launch readiness.
- Use Steam Next Fest at most once and choose the edition deliberately; current
  rules allow each title only one participation and require a public store page
  plus playable demo:
  [Steam Next Fest](https://partner.steamgames.com/doc/marketing/upcoming_events/nextfest).

### Stage 3 — paid Early Access or release

- Launch Steam and direct desktop close enough together that neither audience
  receives a stale or materially inferior product.
- Launch Quest in the same campaign only if its onboarding, comfort,
  persistence, multiplayer, and frame pacing meet the native store promise.
  A later Quest launch is better than treating headset owners as beta testers
  for a weak port.
- Publish the dedicated server without a graphical launcher and with clear
  compatibility/version support.
- State unfinished systems, update cadence, save compatibility, and support
  expectations honestly if using Early Access.

### Stage 4 — retention and expansion

- Build major updates around reasons to return: world depth, creatures,
  structures, multiplayer, creation, and cross-device play.
- Add localization based on measured audience demand and support capacity.
- Explore supporter editions, soundtrack/art products, hosted services,
  creator tools, mod distribution, or subscriptions only as separately
  justified products.
- Evaluate flat-Android store reach after touch UX and commercial policy are
  proven, not merely because an APK already builds.

## Acquisition And Retention Loops

The initial growth system should use product artifacts rather than paid
advertising assumptions:

- **Wishlist loop:** compelling device-native footage -> Steam page -> demo ->
  wishlist/release notification.
- **World loop:** memorable seed/settlement/challenge -> player attempts it ->
  screenshot, video, or world share -> another player starts.
- **Build loop:** searchable structure/build guide -> interactive preview ->
  open in web demo or game -> modify and share.
- **Cross-device loop:** one world shown on flat desktop, Deck, and Quest ->
  concrete differentiation -> group purchase or invitation.
- **Server loop:** free dedicated server -> durable community world -> player
  invitations and returning sessions.

Do not design virality that requires uploading private worlds, contact lists,
or headset sensor data. Sharing must be explicit and privacy-scoped.

## Success Measures

Track a small funnel by channel rather than celebrating aggregate downloads:

- landing-page source -> Steam wishlist, direct trial, web play, or Quest page;
- demo/trial install -> successful first launch;
- first launch -> first world entered and first meaningful build/craft action;
- first world -> second session, D1, D7, and four-week return;
- demo -> paid conversion by channel and campaign;
- purchase -> refund, crash-free sessions, support contacts, and review score;
- multiplayer invitation -> successful join and later return;
- Steam Deck sessions with no manual configuration;
- Quest sessions meeting comfort and frame-pacing gates; and
- direct-channel share, bandwidth/support cost, and entitlement failures.

A useful product north star is **players who return to a persistent world in a
later week**, segmented by solo and multiplayer. Raw launcher downloads,
catalogue page views, and one-off web sessions are acquisition signals, not
proof of a durable sandbox.

## Release And Marketing Gates

Do not open a public paid offer until the promised flavor has:

- a complete releasable first-party asset set and provenance receipt;
- coherent original capsule art, trailer footage, screenshots, and store copy;
- a controller-complete first session, world creation, settings, pause,
  failure, and recovery path on the advertised device class;
- durable persistence plus explicit save migration/compatibility policy;
- a stable local session and an honest remote-multiplayer support statement;
- release signing, crash handling, update recovery, and support diagnostics;
- privacy policy, ratings/content disclosure, refund/support workflow, and
  store data declarations;
- representative clean-machine installation and previous-release update
  evidence; and
- device-native performance evidence for Deck and Quest claims.

Paid Early Access can ship an incomplete game, but it cannot substitute
potential for a build worth its current price. Steam describes Early Access as
a playable alpha/beta that is worth the current value and is planned to
continue toward release:
[Steam store presence guidance](https://partner.steamgames.com/doc/store).

## Risks And Guardrails

### Brand and expectation risk

Seed/behavior parity creates unusually high expectations if exposed as the
product identity. Keep Minecraft reference materials private, ship original
content, establish a visually distinct world, and describe the actual current
game rather than everything the reference target eventually implies.

### Store and pricing risk

A complete game that costs money in one channel and is free in another can
create review backlash even if formally allowed. Similar value, clear copy,
and no surprise are as important as contractual permission. Do not undercut
Steam merely to advertise the absence of Valve's fee; regional pricing,
discounts, taxes, refunds, and store services make simplistic parity
misleading.

### Entitlement fragmentation

Store receipts, direct licenses, refunds, bans, account links, offline play,
family use, and cross-buy can produce more support burden than the launcher.
Start with channel-native ownership and add linking only for a defined player
benefit.

### Quest category-overconfidence

There is no official Minecraft, but QuestCraft and Discovery 2 prove that the
need is visible and already served partially. The opportunity is not protected
by the gap. Store quality, 72 FPS behavior, comfort, onboarding, content depth,
and social reliability determine whether Mclone is a product rather than
another sideload experiment.

### Multi-platform launch dilution

Five first-class engine targets do not require five simultaneous commercial
launches. Product behavior stays shared, while marketing and support can stage
channels according to evidence. Do not degrade the architecture to stage the
business, and do not overextend support merely to advertise every build at
once.

### Youth, UGC, and privacy

Voxel sandboxes and Quest can attract younger players. Accounts, multiplayer,
chat, shared structures/worlds, moderation, telemetry, camera/passthrough, and
payment flows require deliberate age-rating, parental, privacy, and safety
design before they become marketing features.

## Decisions Recorded

- Steam is preferred; the strategy must not steer Steam users to direct
  payment.
- People who prefer not to use Steam should have a supported first-party
  desktop option.
- Quest Store distribution and platform updates are desirable, with signed
  sideload access retained for users/testers who knowingly choose it.
- The game should remain behaviorally the same across paid/store/direct
  flavors; monetization is not a gameplay fork.
- A direct free trial plus first-party payment is a serious and currently
  cleaner candidate than charging on Steam while making the entire direct game
  free.
- Steam should use its own paid entitlement and demo shape rather than an
  external lightweight license/paywall inside the Steam build.
- Standalone Quest is a strategic differentiation wedge because official
  Minecraft VR support ended and the remaining routes are workaround or
  narrower-competitor products.
- Public positioning must be original and product-led, not “Minecraft clone”
  or 1.17.1 parity marketing.

## Open Decisions

- Is the final model paid-per-channel with trials, complete free direct plus
  paid supporters, or free base plus supporter products?
- What price and regional-pricing posture matches the launch content, Early
  Access state, competitive set, and support burden?
- Does one direct purchase cover every direct desktop/Android/XR build, and how
  many people or devices does it cover?
- Is an Mclone identity optional, required only for online services, or also an
  entitlement recovery mechanism?
- Are cross-save and any form of cross-buy worth the policy and support cost at
  launch?
- What bounded trial content best predicts purchase without misrepresenting
  the open-ended game?
- Should Steam/direct PC launch before Quest, or can Quest meet the same
  campaign window without compromising native quality?
- Which Meta trial, testing, IAP, external-entitlement, and package-identity
  options are actually approved under the agreement in effect at submission?
- Which payment processor or merchant of record owns direct tax, refunds,
  chargebacks, invoices, and regional availability?
- Is the public dedicated server always free, and which commercial hosted
  service, if any, is deliberately separate from it?
- What mod, UGC, chat, creator-marketplace, or shared-world promises are in the
  first public release versus explicitly later?

## Evidence To Gather Next

Before settling price or launch order:

1. Conduct a fresh paid-product review of Minecraft, Hytale, Vintage Story,
   Discovery 2, cyubeVR, Luanti game distributions, and other current Steam /
   Quest voxel-survival releases.
2. Record comparable price, review volume/sentiment, demo/trial shape,
   platforms, controller quality, update cadence, and recurring complaints.
3. Obtain written Steam and Meta answers for the exact direct/store pricing,
   entitlement, and external-account model under consideration.
4. Prototype the same bounded first-session demo in web, direct desktop, and
   Steam packaging, then measure completion and return behavior.
5. Run moderated onboarding tests on ordinary PC, Steam Deck, and Quest with
   players who did not install or build the game.
6. Estimate direct-channel economics: payment/tax fees, bandwidth, support,
   refunds, chargebacks, signing, CDN, and entitlement operations.
7. Test two positioning cuts—broad cross-platform survival versus standalone
   Quest/Deck access—against wishlist, trial, and second-session behavior.

## Related Documents

- [`release-distribution-and-updates.md`](release-distribution-and-updates.md)
  — launcher, updater, signing, manifest, artifact, and install ownership.
- [`../platforms.md`](../platforms.md) — current platform posture and
  validation matrix.
- [`platform-parity.md`](platform-parity.md) — feature and shared-contract
  parity across targets.
- [`controller-input.md`](controller-input.md) — gamepad, Deck, tracked-XR,
  glyph, and Steam Input direction.
- [`performance.md`](performance.md) — measured platform performance work and
  Quest guardrails.
- [`asset-pack-profiles.md`](asset-pack-profiles.md) — first-party public asset
  packs and provenance boundary.
- [`structure-catalogue-product.md`](structure-catalogue-product.md) —
  catalogue-led acquisition and product-economy exploration.
- [`../native-web.md`](../native-web.md) — web build and deployment mechanics.
