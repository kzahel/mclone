# Isocraft Reveal Reference

Topic: `isocraft-reference`

Status: **RuneFist Isocraft 0.1.1 for Minecraft 1.21.11 Fabric was
inspected on 2026-08-22 from its public description, release notes, published
jar, shipped shaders, and decompiled classes.** Its reveal is a topology-first
hybrid: classify the player's connected interior, choose a whole-room or
bounded local-cave mask, then remove only camera-side occluders that can prove
real visible space behind them. This materially strengthens the cutaway
direction in [`tabletop-overview-mode.md`](tabletop-overview-mode.md), but it
does not authorize an implementation or tactical.

## Scope And Short Answer

This note answers how
[Isocraft](https://modrinth.com/mod/isocraft) decides when and where to reveal
terrain around a player using its isometric camera, especially in rooms and
caves. It also records the product lesson for a possible Mclone overview or
direct-control adventure mode intended to reduce dependence on first-person
camera skill.

The compact answer is:

```text
player cell and nearby topology
  -> find a roofed, passable seed
  -> bounded flood of connected air/liquid
  -> add its one-cell solid boundary shell
  -> classify result
       +-- credible bounded room -> room/dollhouse reveal
       +-- buried or oversized cave -> local-cave reveal
       `-- open to nearby sky -> no interior reveal
  -> test camera-side walls/roof/overburden against that mask
  -> remove proven occluders, darken everything outside the mask
  -> make interaction rays skip the same removed presentation geometry
```

It is not primarily a ray from the camera to the player. Camera rays are a
secondary occluder and safety test. The connected-space mask is what stops a
cave mouth from exposing an entire cave network and stops an exterior hill
from being mistaken for the player's room.

## Specimen And Source Quality

| Field | Receipt |
|---|---|
| Project | [Modrinth project](https://modrinth.com/mod/isocraft), ID `LVdphWEu` |
| Inspected version | [`0.1.1+1.21.11-fabric`](https://modrinth.com/mod/isocraft/version/i44Uzr4s), published 2026-07-23 |
| Artifact | `isocraft-0.1.1+1.21.11-fabric.jar` |
| SHA-512 | `7c0d10c530491cb605c9b6bd0e96850092b3c0624d2c1aa0fa6d61d476a9481dbedea27d38b7c0840f64002bce3d30098ffd15b7072f3c9411553d617821b02b` |
| Inspection tool | CFR 0.152 |
| Public source | No source URL or source artifact exposed by the project/version metadata |
| License receipt | Modrinth says All Rights Reserved; the jar declares MIT in `fabric.mod.json` and contains an MIT `LICENSE_isocraft` |

The ignored local specimen lives under
[`reference/isocraft/`](../../reference/isocraft/README.md). It retains the
published artifact, exact Modrinth API responses, extracted resources and
license, and CFR output. Because the public license metadata and bundled
license disagree, this study preserves both and treats the decompilation as a
local research aid rather than a source to copy. CFR can also reconstruct
complex control flow imperfectly; conclusions below rely on responsibilities
and repeated data flow across the producer, mask, renderer mixins, shaders,
and targeting path.

## 1. It First Decides Whether The Player Is In A Space

The main scanner is
[`OcclusionProducer`](../../reference/isocraft/decompiled-1.21.11-fabric/com/runefist/isocraft/occlusion/OcclusionProducer.java).
Its ordinary scan has these bounds:

- a fixed `48 x 24 x 48` bit mask around the player;
- configurable horizontal interior radius clamped to `8..22`, default `20`;
- `15` interior cells above and `6` below the player's feet;
- a one-cell shell around the interior bounds; and
- a hard flood budget of `12,000` processed cells.

A candidate cell must be physically passable and have a passable vertical
partner. The scanner then looks upward toward the motion-blocking heightmap and
classifies the column as roofed, open sky, or tall but not yet resolved. Roofed
and unresolved-tall cells can continue a bounded flood. Open-sky cells stop it.

Doors, fence gates, and trapdoors are intentional separators even while open.
The scanner records them as visible portal boundaries but does not flood
through them. When the player stands in a portal, it prefers a roofed neighbor
that belonged to the previous interior, which makes crossing a doorway change
rooms instead of briefly merging both sides. Fence gates extend a bounded
vertical virtual curtain so an open gate does not leak over its one-block
shape.

The full scan records two bitsets:

- **interior**: connected passable air/liquid that belongs to the selected
  room; and
- **visible**: that interior plus its immediate solid/portal boundary shell.

That distinction later lets the renderer prove that a candidate wall really
borders the room rather than merely occupying the same broad bounding box.

## 2. A Structural Classifier Rejects Accidental Rooms

A bounded flood alone is not enough. A tree canopy, arch, cave mouth, or
partially roofed outdoor path can all produce locally roofed cells. Isocraft
therefore samples roof, wall-sector, floor-support, and overburden coverage.
[`StructuralRoomClassifier`](../../reference/isocraft/decompiled-1.21.11-fabric/com/runefist/isocraft/occlusion/StructuralRoomClassifier.java)
combines them with separate entry and retention thresholds.

The classifier accepts several useful shapes:

- a strongly roofed, floor-supported room with enough wall coverage, including
  an open-front build;
- a strongly weighted structural enclosure; or
- while retaining an existing mask, a weaker but still coherent edge case or
  cave-like space.

The lower retention threshold plus a two-scan enter/exit vote provides
hysteresis. Topology changes, movement out of the current interior, local
recenter thresholds, and a periodic 200-tick fallback can trigger rescans.
This is why the result is much steadier than making a fresh binary decision
from the current camera ray every frame.

## 3. Large Or Deep Caves Use A Different Flood

The ordinary room scan deliberately falls back when it reaches its work budget
or a synthetic scan edge while the space still looks enclosed. It can also
select the cave path when room classification fails but the player is clearly
embedded in terrain.

The buried-player heuristics in
[`LocalCavePolicy`](../../reference/isocraft/decompiled-1.21.11-fabric/com/runefist/isocraft/occlusion/LocalCavePolicy.java)
combine:

- heightmap cover around the eight neighboring columns;
- real structural overburden, not height alone;
- a supported floor within a few cells below the player;
- nearby body-height cave walls or thick neighboring cover; and
- a passable two-cell body/shaft seed.

The local-cave scan uses an `11`-cell X/Z radius but limits connected reveal to
at most `10` horizontal steps. Vertical moves cost zero in its small geodesic
search, so a shaft can remain coherent without allowing a winding horizontal
cave to expose arbitrary distant branches. It includes a one-cell apron at the
bounded edge so the result ends on real terrain rather than a naked void.

An open-sky check rejects the local cave if sky is reachable within four
topological steps. A retained cave mask is periodically retried as a full room
scan after the player moves far enough, so the bounded fallback is not a
permanent classification.

This separate local-cave mode is the most reusable idea in the implementation.
“Connected to the player” and “safe to show in full” are different predicates.

## 4. It Separates Visibility From Geometry Removal

The mask is not itself a destructive cut. It first controls presentation:

- fragments inside the visible mask retain their normal material;
- terrain outside it is multiplied toward a configurable exterior brightness,
  defaulting to black; and
- entities, block entities, and later interaction tests use the same shown
  mask.

Geometry between the camera and that visible space is then removed through a
hybrid renderer path:

1. CPU code selects camera-facing room walls, ceilings, shadowed overburden,
   and camera-safety cells.
2. Vanilla and Sodium mesh hooks substitute selected blocks with air during
   section meshing, which exposes the already-existing faces behind them.
3. The terrain shader can additionally discard a foreground fragment only
   when a bounded ray march finds an actual visible-mask cell behind it.
4. A post pass reconstructs world position from depth and darkens outside-mask
   pixels for replacement renderers that bypass the custom terrain shader.

The mesh path tracks render generations and waits for affected sections to be
presented before retiring old geometry or advancing large camera changes.
That machinery is mod-specific, but the general lesson is sound: reveal state
and displayed geometry need an acknowledgement boundary or camera rotation can
briefly expose stale walls, missing backing faces, or X-ray holes.

## 5. Local Caves Get A Smaller, Proven Aperture

Rooms may remove the camera-facing wall and roof across the room's real shape.
Local caves do not. The shipped
[`terrain.fsh`](../../reference/isocraft/extracted/assets/isocraft/shaders/core/terrain.fsh)
uses a bounded camera-to-player elliptical viewport with hard camera and player
planes.

There are three important guards:

- **Exterior overburden** may use a projection-aware aperture that widens from
  a small lens neck toward the player, but only if a short ray finds the local
  interior or its truthful one-cell shell behind the candidate.
- **Authored cave shell** uses a much smaller fixed entry aperture or a
  character-sized silhouette test. A nearby side wall cannot authorize its
  own removal merely because it lies inside the broad aperture.
- **Navigable floor and low terrain** are preserved. Solid terrain below the
  player's floor band, and horizontal floor faces the player may need to read,
  do not disappear. Alpha-cutout vegetation and similar non-solid materials do
  not inherit the rock aperture.

The transition uses ordered dithering and smoothed reveal strength rather than
making a whole new translucent terrain layer. At long zoom, the viewport is
projection-aware and bounded; in tiny pockets a separately guarded fallback
keeps the body readable. The Sodium/replacement-renderer path expresses the
same purpose through a wider bounded mesh aperture because its shader
capabilities differ.

Finally,
[`IsoTargeting`](../../reference/isocraft/decompiled-1.21.11-fabric/com/runefist/isocraft/input/IsoTargeting.java)
repeats the relevant visibility tests while raycasting. It skips blocks that
the mesh cut or shader has actually dissolved. That avoids the classic failure
where the cursor still selects an invisible roof.

## Product Lesson For Mclone

An optional external-camera adventure mode is a strong fit for Mclone. It can
lower the requirement to coordinate mouse-look and movement, make the player's
body continuously legible, and provide a natural flat-screen, touch, gamepad,
and tabletop-XR expression of one canonical world. Isocraft's public feature
set also demonstrates that a complete alternative is more than a camera: it
pairs screen-relative movement, cursor aim, optional click-to-move, vertical
targeting, hidden-face building assistance, and contextual feedback with the
reveal.

It should be framed as an opt-in **Adventure View** or overview control profile,
not as a simpler game mode with different authority. The same player body,
collision, survival rules, reach, inventory, persistence, and multiplayer
simulation should remain canonical. A player can switch presentations without
changing what actions the server permits.

It is accessibility-adjacent rather than universally accessible. Isometric
depth, vertical targeting, hidden block faces, and camera-relative movement
introduce their own learning costs. The direction should be validated with
people who do not already use first-person controls; click-to-move, cursor
actions, optional snap rotation, strong target previews, and automatic camera
follow are likely more important to that audience than exact isometric
projection.

## Recommended Mclone Direction

The existing tabletop topic proposed a DDA-triggered shader keyhole. This study
shows that the DDA is useful but too weak as the primary decision. The better
shared contract is:

```text
bounded player-local topology mask
  + room / local-cave / outdoor classification
  + camera occluder proof
  + one visibility rule shared by rendering and picking
```

Mclone should implement that policy in a shared, host-neutral owner when the
feature is selected. Platform code should only supply camera/input events and
presentation targets. The visibility mask is client presentation state over
ordinary replicated world data; it must not alter canonical blocks or server
collision.

Do not copy Isocraft's constants or Minecraft mixin architecture. Mclone owns
its renderer and can define a cleaner mask/revision contract, reuse chunk
topology already known to the client, and test the state machine directly. It
also has an XR requirement Isocraft does not prove: the cut must be one physical
world-space volume shared by both eyes and supported by per-eye and full-frame
multiview rendering. Independent screen-space holes are not acceptable.

The first selected slice should still follow the read-only overview foundation
in [`tabletop-overview-mode.md`](tabletop-overview-mode.md). A bounded flat
direct-control follow-up can then prove screen-relative movement, local-player
presentation, room/local-cave classification, render/picking agreement, and
the ordinary embodied path remaining unchanged.
