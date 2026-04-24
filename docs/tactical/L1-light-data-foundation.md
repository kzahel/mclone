# L1 - Light data foundation

Standing after [`L0-lighting-oracle-foundation.md`](L0-lighting-oracle-foundation.md) and the durable reference in [`../lighting.md`](../lighting.md). This slice gives the runtime a vanilla-shaped place to carry stored light before porting propagation.

## Goal

Land the light data primitives and packet-friendly snapshot shape needed by the future `LevelLightEngine` port:

- port vanilla `DataLayer` as the 2048-byte, 4096-nibble section store
- expose `LightLayer` source-surrounding defaults
- model light-section padding separately from block-section ranges
- carry optional sky/block `DataLayer` bytes through `ChunkSnapshot`, `PackedChunkSnapshot`, transferables, and remote wire JSON
- prove the shape with the committed L0 official-server fixture

At the end of `L1`, we can move vanilla light bytes through the runtime data path without interpreting them yet.

## Reference source

Read these before implementing:

| Java source | Why it matters |
|---|---|
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/chunk/DataLayer.java` | exact nibble layout, lazy allocation, copy behavior, and debug strings |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/LightLayer.java` | sky/block layer names and source-surrounding defaults |
| `reference/minecraft-1.17.1/src/net/minecraft/world/level/lighting/LevelLightEngine.java` | light-section padding around normal world sections |
| `reference/minecraft-1.17.1/src/net/minecraft/core/SectionPos.java` | section-Y addressing model used by light sections |

## Scope

| # | Module | Result |
|---|---|---|
| 1 | `src/world/level/chunk/data-layer.ts` | direct `DataLayer` port with vanilla nibble layout and lazy storage |
| 2 | `src/world/level/light-layer.ts` | `SKY`/`BLOCK` layer enum plus `getLightLayerSurrounding` |
| 3 | `src/world/level/light-section.ts` | `minSection - 1`, `sectionCount + 2`, and section index/Y helpers |
| 4 | chunk snapshot types | optional `ChunkLightSnapshot` with sky/block section byte arrays and `lightCorrect` |
| 5 | packed snapshot codecs | clone, pack/unpack, transferable collection, and 2048-byte validation for light sections |
| 6 | remote wire codec | base64 JSON encoding/decoding for packed light bytes |
| 7 | tests | unit coverage plus L0 fixture roundtrip through packed snapshots |

## Explicit non-goals

- no `DynamicGraphMinFixedPoint` or `LevelLightEngine` port
- no authoritative `WorldLightEngine` service yet
- no render lighting mode switch yet
- no client render-world light cache
- no mesh packed-light replacement
- no light deltas or live edit propagation
- no browser visual validation

## Implementation status

L1 is landed:

- `DataLayer` stores two vanilla light nibbles per byte using index order `y << 8 | z << 4 | x`.
- `LightLayer.SKY` reports source-surrounding light `15`; `LightLayer.BLOCK` reports `0`.
- light-section helpers preserve vanilla's one-section padding above and below the normal world section range.
- `ChunkSnapshot` and `PackedChunkSnapshot` can carry optional sky/block light sections and `lightCorrect`.
- packed snapshot clone/transferable helpers include light byte buffers.
- remote packed-chunk wire JSON serializes light sections as base64 and validates every decoded section is exactly `DataLayer.SIZE` bytes.
- the committed L0 fixture now round-trips its vanilla light bytes through packed snapshot conversion.

## Validation

Focused validation:

```bash
pnpm test -- test/world/data-layer.test.ts test/world/light-section.test.ts test/world/packed-chunk-snapshot.test.ts test/runtime/packed-chunk-wire.test.ts
pnpm typecheck
```

Coverage added:

- `DataLayer` lazy reads, nibble order, value masking, constructor validation, copy behavior, and debug string shape
- light-section padding and index/Y conversion
- L0 fixture light byte preservation through `packChunkSnapshot` / `unpackChunkSnapshot`
- clone isolation and transferable inclusion for light byte buffers
- remote wire base64 roundtrip and invalid light length rejection

## Next

`L2` should port the pure solver foundation:

- `DynamicGraphMinFixedPoint`
- `LayerLightSectionStorage`
- `LayerLightEngine`
- `BlockLightEngine`
- `SkyLightSectionStorage`
- `SkyLightEngine`
- `LevelLightEngine`

Keep this as a source-parity port first, backed by a small test-level adapter and the L0 fixture bytes before changing render-world or meshing.
