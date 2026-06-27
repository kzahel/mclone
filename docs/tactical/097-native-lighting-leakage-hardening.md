# Native Lighting Leakage Hardening

Status: active; slices 1-3 first pass landed.

## Context

Underground tunnels currently appear brighter than vanilla Minecraft Java 1.17.1. The user-visible symptom is not forced fullbright: toggling the render fullbright comparison still leaves unexpected bright underground surfaces in the lit path. The most likely immediate cause is sky-light leakage at packed snapshot/render boundaries where a Java-shaped explicit empty `DataLayer` is collapsed into "no layer".

Reference files:

- `reference/minecraft-1.17.1/src/net/minecraft/network/protocol/game/ClientboundLightUpdatePacket.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java`
- `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/SkyLightSectionStorage.java`
- `native/crates/mclone-light/src/data_layer.rs`
- `native/crates/mclone-light/src/packed.rs`
- `native/crates/mclone-server/src/light_world.rs`
- `oracle/lib/integration/light-fixture.ts`
- `oracle/java/net/minecraft/world/level/chunk/McloneSchedulerTraceRecorder.java`

## Vanilla Shape

`DataLayer` may be present while internally empty. Vanilla light packets preserve that distinction with populated and empty masks:

- no bit in either mask means no light layer for that section/layer
- empty mask bit means an explicit all-zero `DataLayer`
- populated mask bit means a 2048-byte nibble array follows

Rendering and client hydration must treat the second case as real data. It is different from an absent layer because missing sky data may be inferred from source sections above, while explicit empty sky data means zero light for that section.

## Implementation Slices

1. Preserve explicit empty sections in packed snapshots.
   - Materialize present empty `DataLayer`s as zero-filled packed layer bytes at the current `PackedLightSection` boundary.
   - Add a regression where a lit section above an explicit dark section does not make the lower section sample sky light.

2. Add generated-chunk lighting oracle fixtures.
   - Use the existing integration light fixture helpers to emit a real chunk or small chunk region after Java `LIGHT` status.
   - Compare native section presence and sky/block bytes against the fixture, including explicit empty sections.
   - Include an underground tunnel/cave case and a lava-lit cave case.
   - First pass landed against the existing persisted Anvil fixture for seed `12345`, chunk `(5,115)`: normalize Anvil-only representation differences, then compare nontrivial sky/block `DataLayer` bytes exactly.
   - This slice also fixed native liquid light opacity: Java `LiquidBlock` does not propagate skylight down, so water/lava attenuate sky by one instead of acting as fully transparent light media.
   - The scheduler oracle now emits a status-aligned `LIGHT` snapshot with live Java light `DataLayer`s. The committed seed `12345`, chunk `(0,0)` fixture is the strict oracle for section masks and sky light.

3. Add live light delta parity.
   - Wire block edits through Java-shaped `checkBlock`/light propagation.
   - Publish light deltas separately from section block deltas.
   - Dirty render sections affected by changed light, not only changed block states.

4. Match Java sky source-section propagation.
   - Keep non-empty sky sections as `LIGHT_AND_DATA` instead of `LIGHT_ONLY` so block opacity participates in source updates.
   - Preserve `LIGHT_ONLY` source behavior for empty sections: fill fullbright, then check horizontal and bottom edges.
   - Mirror Java sky neighbor skip-through across missing vertical sections.
   - Recheck changed retained blocks and make target chunk blocks override stale neighbor dependency snapshots.
   - Approximate Java face occlusion for full-opaque sky source blocks so source-row solid blocks do not leak light through their opaque face.

## First Slice Acceptance

- Packed snapshots preserve explicit zero sky/block layers when the light engine has a visible `DataLayer`.
- Render-side packed light sampling no longer falls through to higher sky layers when the exact section has explicit zero sky data.
- Targeted native lighting tests pass.
- The follow-up remains clear: full generated chunk byte parity and then live edit light deltas.

## Current Gaps

- The persisted-light gate is not yet a full packet-mask parity fixture. Vanilla Anvil omits explicit all-zero layers and may persist full-sky layers that native currently represents through sky fallback.
- Seed `12345`, chunk `(0,0)` now matches the persisted Java oracle for sky light. The ocean fixture at chunk `(5,115)` remains strict for sky and block light.
- Seed `12345`, chunk `(0,0)` now matches the scheduler `LIGHT` fixture for strict sky bytes and the Java light-section envelope. Native synthesizes Java's visible envelope around generated data sections: one section below and two sections above the non-empty block-section range, preserving explicit zero block layers and full-sky upper layers.
- Full strict block-light parity for scheduler `LIGHT` chunk `(0,0)` is one nibble away: section `1`, byte `1784`, expected `0x10`, got `0x00`. Decoded, that is local `(1,29,15)` in chunk `(0,0)`. Keep the ignored strict block gate until the edge-neighbor mismatch is closed.
- Live block edits still publish block-state deltas without light deltas.
