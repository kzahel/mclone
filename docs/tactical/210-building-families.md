# Tactical 210: Bounded Building Families

Topic: `starter-farmstead-settlement`

Status: **complete 2026-07-21. The typed family contract, six core building
members, optional length-matched lean-tos, guarded 81-chunk gallery, seven
focused tests, cross-target checks, and three inspected production captures are
complete.**

## Objective

Turn the accepted Tactical 209 cottage and barn into small, legible authored
families without pretending that arbitrary voxel stretching is good building
design. Preserve the accepted standard forms, introduce only named depth or
length classes and detachable entry/lean-to modules, and inspect all variants
together in an ordinary persisted comparison world.

## Family Contract

- Cottage width, wall height, roof pitch, facade composition, and material
  language remain authored invariants. `snug`, `standard`, and `deep` select
  curated longitudinal plans; `stoop` and `canopy porch` select an authored
  entry module.
- Barn width, gambrel profile, facade, and structural language remain authored
  invariants. `short`, `standard`, and `long` select whole structural bays.
  The east lean-to remains a separately placed template and is optional.
- Callers choose bounded semantic variants, never raw dimensions. Each result
  is still a complete deterministic `StructureTemplate` with honest bounds,
  markers, palette roles, transforms, and placement receipts.
- Existing zero-argument constructors remain the stable accepted standard
  variants. Their template values must remain equal to explicitly requesting
  the standard family member.

This is deliberately a middle ground between one immutable stamp and a general
procedural architecture generator. New widths, roof forms, wings, facade
layouts, or material dialects should become authored sibling families rather
than unchecked numeric knobs.

## Scope

- Add typed cottage depth and entry-module choices.
- Add typed barn length choices and length-matched lean-to templates.
- Keep the Tactical 209 standard cottage, barn, and lean-to constructors and
  gallery intact.
- Add a separate safe-to-rebuild persisted family gallery showing three
  cottages, three barn cores, and lean-tos on only selected barns.
- Add focused determinism, monotonic-dimension, marker, placement, lighting,
  and SQLite reopen tests.
- Capture and inspect production-rendered cottage-family, barn-family, and
  overview images; revise visible regressions before closing the tactical.

## Non-Goals

This slice does not add arbitrary width/height scaling, grammar-selected roofs,
interiors, doors, fences, settlement parcels, terrain adaptation, palette-file
codecs, or automatic variant selection. It does not make the family gallery a
shipping spawn settlement.

## Acceptance Gates

1. The explicit standard members equal the existing convenience constructors.
2. Every named depth/length produces valid deterministic templates with honest
   sizes and markers; increasing a longitudinal class increases its plan and
   block count.
3. The family gallery visibly distinguishes the named classes and demonstrates
   that the lean-to is optional composition, not barn-core geometry.
4. The gallery persists as lit ordinary world chunks, reopens through SQLite,
   and refuses to overwrite unrelated roots.
5. All production captures are inspected before the tactical is closed.

## Validation Ledger

| Date | Evidence | Result |
|---|---|---|
| 2026-07-21 | `cargo test --manifest-path native/Cargo.toml -p mclone-server --lib building_lab` | passed, 7 template/family/gallery/SQLite/protected-root tests |
| 2026-07-21 | `cargo check --manifest-path native/Cargo.toml -p mclone-native-client` | passed |
| 2026-07-21 | `cargo check --manifest-path native/Cargo.toml --target wasm32-unknown-unknown --lib -p mclone-server` | passed; five pre-existing cfg/dead-code warnings remain |
| 2026-07-21 | `building_family_lab_world --root /tmp/mclone-building-family-lab-v1` | wrote 81 lit chunks and 8 placements: cottage cores at 1,082/1,320/1,532 blocks; barn cores at 1,828/2,520/3,212; standard and long lean-tos at 321/411 |
| 2026-07-21 | production `mclone-native-client --screenshot ... --startup-wait frames:180` captures | overview, cottage family, and barn family captured at 1280x720 and inspected |

## Result

The public family inputs are semantic enums rather than raw dimensions.
`CottageDepth` selects three curated longitudinal plans while
`CottageEntry` selects either an accessible stoop or the accepted offset canopy
porch. `BarnLength` adds and removes whole four-block structural bays;
`BarnVariant` decides independently whether to place the matching east
lean-to. The original zero-argument constructors delegate to—and test equal
to—the explicit standard members.

The standard Tactical 209 templates remain visually unchanged. Shorter and
longer members move facades, gables, roof runs, structural lines, interiors,
and markers together instead of scaling block coordinates. The deep cottage
has a specifically authored second side-window bay so its extra length does not
become blank plaster. Lean-tos remain separate placed templates with lengths
matched to their barn cores.

The separate `bounded-building-families-v1` world places three cottages, three
barn cores, and lean-tos on only the standard and long barns. Its guarded
writer preserves the original accepted gallery, refuses unrelated non-gallery
roots, records ordinary placement receipts, lights every chunk, and aligns its
spawn with a simple comparison path.

## Visual Review Receipt

Final captures live outside the repository:

- `/tmp/mclone-building-family-overview.png`;
- `/tmp/mclone-building-family-cottages.png`; and
- `/tmp/mclone-building-family-barns.png`.

The first family layout made depth difficult to compare because an oblique
camera compressed the cottage row. Centering that row around the gallery axis
produced a stable straight-on comparison: the snug stoop, accepted standard
porch, and deeper two-side-window plan read as siblings without appearing to be
uniform scale operations. The elevated barn view makes the short core's missing
lean-to, the accepted standard composition, and the longer extra-bay form
simultaneously legible. A 180-frame explicit screenshot warmup was used for the
final review so all asynchronously compiled persisted sections were present.

This result is intentionally not a general building grammar. Widths, heights,
roof forms, and facade dialect remain authored; additional choices should be
named sibling families or modules. The next likely authoring improvement is an
external textual/captured voxel record, while true generated settlement
placement still depends on `FS-04` and later overlay/site-plan work.
