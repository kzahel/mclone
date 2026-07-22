# Figure Binary Alpha Cutout

Topic: `figure-alpha-cutout`

Status: implemented and visually approved on 2026-07-22 across Asset Lab and
the shared native prepared-figure path. The accepted follow-up for partial
alpha and Ghost rendering is
[`figure-transparency-materials.md`](figure-transparency-materials.md).

## Scope

This topic owns fully transparent texels in Asset Lab ASCII palettes and their
consistent binary-cutout interpretation through semantic JSON, Three.js,
prepared atlases, actor rendering, and mono/stereo/multiview shaders.

It does not define `#RRGGBBAA`, fractional opacity, blended materials,
order-independent transparency, ghost styling, or translucent shadow policy.
Those need a separate material/pass decision after the cutout tool is proven.

## Contract

- Palette values are opaque `#RRGGBB` or the exact literal `"transparent"`.
- Partial-alpha hex colors are rejected instead of being silently rounded to
  opaque or cutout.
- Transparent pixels become `[0, 0, 0, 0]` in the shared prepared atlas.
- Samples with alpha below `0.1` are discarded. Opaque samples render through
  the ordinary opaque pass with blending disabled and depth writes enabled.
- The same rule applies to the semantic Three.js preview, prepared-figure
  mono and multiview proofs, and the actor mono, per-eye, placed, and multiview
  shaders.
- A transparent glyph has no implicit meaning. Authors opt in by assigning
  that glyph the explicit `"transparent"` palette value.

This is Minecraft-style texture cutout behavior, not translucency. It avoids
sorting ambiguity and prevents invisible torso faces from occluding geometry
behind them.

## Visual Proof

`examples/cutout_skeleton/figure.ts` is a second, deliberately distinct
Skeleton. Its torso remains one full cuboid; front, back, side, top, and bottom
textures carve the rib cage using transparent palette texels. The original
Skeleton remains the deeper separate-box rib construction, giving the
catalogue an immediate geometry-versus-cutout comparison.

The same review slice shortens and tucks Gargoyle's upper arms, forearms, and
claws so its wings, torso, and tail remain legible in front, side, and
three-quarter animation views. That correction is ordinary geometry and does
not depend on alpha.

## Evidence And Validation

- Semantic validation tests prove transparent palette JSON round-trips and
  `#ffffff80` is rejected.
- Rust atlas tests prove transparent texels and their padding remain zero
  alpha after preparation.
- Static shader coverage proves both prepared-figure mono and multiview paths
  contain the cutout discard; existing actor shader variants use the same
  threshold.
- The direct semantic-versus-native prepared comparison at
  `/tmp/mclone-asset-lab/binary-cutout/native-compare/comparison.png` shows the
  same open rib gaps in front, side, and three-quarter views.
- Clean sheets and MP4 contact strips under
  `/tmp/mclone-asset-lab/binary-cutout/` show stable holes during Hollow March
  and Hollow Rattle, plus the corrected Gargoyle silhouette during Stone
  Stalk, Night Glide, and Stone Awaken.
- The required geometry, ground, and surface scans retain zero failures across
  174 canonical figures.

## Deferred Direction

The follow-up selected dithered mask coverage, a bounded character blend
depth-prepass, additive layers, and explicit material/pass bucketing. Its
contract and evidence now live in
[`figure-transparency-materials.md`](figure-transparency-materials.md); exact
order-independent transparency remains deferred there.
