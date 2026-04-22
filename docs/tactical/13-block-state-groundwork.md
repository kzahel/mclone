# Tactical 13 — Block state groundwork

Port the CPU-side block/state/property layer that model loading and baking depend on: `Property`, `StateHolder`, `StateDefinition`, `BlockBehaviour`, `Block`, `BlockState`, and the small block hierarchy pieces that carry directional state. This slice is pure data and rules, with no renderer or world simulation attached yet.

## Source files (read before writing)

| Java / asset source | TS target |
|---|---|
| `reference/.../src/net/minecraft/world/level/block/state/properties/Property.java` | `src/world/level/block/state/properties/property.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/BooleanProperty.java` | `src/world/level/block/state/properties/boolean-property.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/IntegerProperty.java` | `src/world/level/block/state/properties/integer-property.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/EnumProperty.java` | `src/world/level/block/state/properties/enum-property.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/DirectionProperty.java` | `src/world/level/block/state/properties/direction-property.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/properties/BlockStateProperties.java` | `src/world/level/block/state/properties/block-state-properties.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/StateHolder.java` | `src/world/level/block/state/state-holder.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/StateDefinition.java` | `src/world/level/block/state/state-definition.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/BlockBehaviour.java` | `src/world/level/block/state/block-behaviour.ts` |
| `reference/.../src/net/minecraft/world/level/block/state/BlockState.java` | `src/world/level/block/state/block-state.ts` |
| `reference/.../src/net/minecraft/world/level/block/Block.java` | `src/world/level/block/block.ts` |
| `reference/.../src/net/minecraft/world/level/block/DirectionalBlock.java` | `src/world/level/block/directional-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/HorizontalDirectionalBlock.java` | `src/world/level/block/horizontal-directional-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/RotatedPillarBlock.java` | `src/world/level/block/rotated-pillar-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/AirBlock.java` | `src/world/level/block/air-block.ts` |
| `reference/.../src/net/minecraft/world/level/block/Rotation.java` | `src/world/level/block/rotation.ts` |
| `reference/.../src/net/minecraft/world/level/block/Mirror.java` | `src/world/level/block/mirror.ts` |
| `reference/.../src/net/minecraft/world/item/context/BlockPlaceContext.java` | `src/world/item/context/block-place-context.ts` |
| `reference/.../src/net/minecraft/world/level/BlockGetter.java` | `src/world/level/block-getter.ts` |
| `reference/.../src/net/minecraft/world/level/material/Material.java` | `src/world/level/material/material.ts` |
| `reference/.../src/net/minecraft/world/level/material/MaterialColor.java` | `src/world/level/material/material-color.ts` |
| `reference/.../src/net/minecraft/world/level/material/PushReaction.java` | `src/world/level/material/push-reaction.ts` |
| `reference/.../src/net/minecraft/world/level/block/SoundType.java` | `src/world/level/block/sound-type.ts` |
| `reference/.../src/net/minecraft/world/level/block/RenderShape.java` | `src/world/level/block/render-shape.ts` |
| `reference/.../src/net/minecraft/util/StringRepresentable.java` | `src/core/string-representable.ts` |
| `reference/.../src/net/minecraft/core/Direction.java` | `src/core/direction.ts` |
| `reference/.../src/net/minecraft/core/Vec3i.java` | `src/core/vec3i.ts` |
| `reference/.../src/net/minecraft/core/BlockPos.java` | `src/core/block-pos.ts` |

## What to port and what to skip

### Direct translation (no divergence)

- `Property` value naming/parsing, cached hash code, and the concrete `BooleanProperty`, `IntegerProperty`, `EnumProperty`, and `DirectionProperty` subclasses.
- `StateHolder` immutable property map semantics: `getValue`, `setValue`, `cycle`, neighbor-table population, and string formatting.
- `StateDefinition` / builder validation: legal property names, legal serialized value names, duplicate-property rejection, and the cartesian-product state expansion.
- `BlockBehaviour.Properties`, `BlockBehaviour.BlockStateBase`, `Block`, `BlockState`, and the directional block subclasses needed for future model-shaper logic.
- `Direction`, `Rotation`, `Mirror`, `Material`, `MaterialColor`, `PushReaction`, `SoundType`, `RenderShape`, `Vec3i`, and `BlockPos` only as far as the state/property layer consumes them.

### Intentionally deferred in this slice

- Voxel shapes, collision logic, fluid state, loot, light emission hooks beyond the basic stored fields, and block interaction/gameplay methods not needed by model parsing or baking.
- Rich world access. `BlockGetter` is only the minimal interface tactical 14a/14b will need for state-driven model lookup.

## Oracle / done-when

**Unit (Vitest, no browser):**

- `BooleanProperty`, `IntegerProperty`, `EnumProperty`, and `DirectionProperty` preserve Java-style serialized names and allowed-value sets.
- `StateDefinition.Builder` rejects invalid property names and duplicate properties.
- `StateHolder.cycle(...)` and `Block.withPropertiesOf(...)` produce the expected neighbor states for a small test block.
- `RotatedPillarBlock` defaults to axis `Y`, derives axis from placement face, and swaps `X/Z` on 90-degree rotations.
- `HorizontalDirectionalBlock` rotates and mirrors its `FACING` property exactly like the Java code.

## Done when

- `pnpm test` passes
- `pnpm typecheck` passes
- `docs/tactical/README.md` links this slice and marks tactical 13 as done

## Next

Tactical 14a: [`14a-block-model-unbaked-graph.md`](14a-block-model-unbaked-graph.md) — block model JSON parse and unbaked model graph. Port `BlockModel`, `BlockElement`, `BlockElementFace`, `BlockFaceUV`, parent-model resolution, and texture-variable resolution so model baking can consume real blockstate/model JSON.
