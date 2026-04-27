# EntityRender0 - Vanilla-shaped entity renderer foundation

Status: **in progress**. The cleanup slice removes the standalone player renderer; the next implementation slice is `MultiBufferSource` / `RenderBuffers`.

This replaces the old placeholder-first rendering direction from [`Creatures3-render-entity-placeholders.md`](Creatures3-render-entity-placeholders.md). Remote player snapshots are useful protocol data, but player rendering must not land as a standalone Steve-specific WebGPU renderer.

## Goal

Build the first browser entity rendering path with the same shape as Minecraft Java 1.17.1:

```text
LevelRenderer
  -> EntityRenderDispatcher
  -> EntityRenderer / LivingEntityRenderer / PlayerRenderer
  -> EntityModel / ModelPart
  -> MultiBufferSource / VertexConsumer
  -> RenderType batches
```

The first visible target is a neutral-pose remote player. Animation, equipment, name tags, capes, held items, and armor are follow-up work.

## Source Review

Read these before implementing the slice:

| Concern | Vanilla source |
|---|---|
| Entity render loop, culling, camera-relative coordinates, batch flush order | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/LevelRenderer.java` |
| Renderer lookup and shared entity render entrypoint | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/EntityRenderDispatcher.java` |
| Base renderer contract, culling, light lookup, name tags | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/EntityRenderer.java` |
| Living-entity transforms and model/layer render sequence | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/LivingEntityRenderer.java` |
| Player renderer, model selection, skin parts, layers | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/player/PlayerRenderer.java` |
| Player and humanoid model definitions | `reference/minecraft-1.17.1/src/net/minecraft/client/model/PlayerModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/model/HumanoidModel.java` |
| Model parts and cube-to-vertex emission | `reference/minecraft-1.17.1/src/net/minecraft/client/model/geom/ModelPart.java` |
| Mesh/layer baking helpers | `reference/minecraft-1.17.1/src/net/minecraft/client/model/geom/builders/` |
| Batched entity vertex buffers | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/MultiBufferSource.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/RenderBuffers.java` |
| Entity render types and textures | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/RenderType.java` |
| Entity GLSL shader semantics to port to WGSL | `reference/minecraft-1.17.1/src/assets/minecraft/shaders/core/rendertype_entity_translucent.*` |

## Architecture Divergence Review

Vanilla mutates OpenGL state through `RenderType` setup/clear hooks and binds textures through `TextureStateShard`. `mclone` must lower those render states to WebGPU pipeline descriptors and bind groups. That divergence is already sanctioned for renderer code. The scope here is narrow:

- keep `RenderType` as the semantic batch key
- include texture/resource identity in entity render types so WebGPU bind groups can bind the correct skin texture
- keep vertex data in `DefaultVertexFormat.NEW_ENTITY`
- keep per-frame entity geometry generation through `ModelPart.render(...)` and `VertexConsumer`
- do not create a separate renderer path that bypasses `RenderType`, `PoseStack`, `ModelPart`, `MultiBufferSource`, lightmap, or overlay data

This makes future parity easier because every later entity renderer can reuse the same dispatcher, model, buffer, texture, light, and render-type path.

## Scope

| # | Work | Expected result |
|---|---|---|
| 0 | Remove standalone GPU player renderer | no Steve-specific pipeline, shader, smoke script, or `encodeSceneFrame` hook remains |
| 1 | Port `MultiBufferSource.BufferSource` and renderer-owned `RenderBuffers` | entity renderers can request `VertexConsumer`s by `RenderType` and flush batches |
| 2 | Add entity `RenderType`s | `entityTranslucent`, `entityCutout`, `entityCutoutNoCull`, and `entitySolid` use `DefaultVertexFormat.NEW_ENTITY` and texture-specific state |
| 3 | Add entity shader support | WGSL ports cover the first player skin path, including UV0, UV1 overlay, UV2 lightmap, color, normal, fog, alpha discard |
| 4 | Port model geometry foundation | `ModelPart`, cube builders, layer definitions, and baked model-layer roots support `PLAYER` and `PLAYER_SLIM` first |
| 5 | Port first renderer stack | `EntityRenderDispatcher`, `EntityRenderer`, `LivingEntityRenderer`, `PlayerRenderer`, `HumanoidModel`, and `PlayerModel` render neutral-pose remote players |
| 6 | Integrate with `LevelRenderer` | entity batches are collected in `LevelRenderFrame` alongside chunk `layerDraws`; `encodeSceneFrame` draws batches generically |
| 7 | Validate pixels | smallest browser or Deno WebGPU smoke captures and inspects a remote player rendered through the dispatcher path |

## Protocol Input

Remote players should remain normal entity snapshots:

- `typeId: "minecraft:player"`
- category `misc`
- vanilla dimensions `0.6 x 1.8`
- host-authoritative position and rotation
- data may carry `playerId`, `name`, skin model (`default` or `slim`), skin texture location, and visible model parts

Renderer dispatch must use `typeId`; any `data.kind` field is metadata, not the renderer switch.

## Deferred

- limb animation and interpolation-derived walk cycle
- head/body yaw separation beyond current snapshot rotation
- crouching, swimming, sleeping, fall flying, riding, attack poses
- armor, held items, arrows, bee stingers, cape, elytra, ears, parrots
- name tags, outlines, shadows, hitboxes
- entity removal/delta protocol beyond current snapshot/unload behavior
- full `AbstractClientPlayer` / skin profile behavior

## Validation

For each rendering slice:

- `pnpm typecheck`
- focused unit tests for pure model/buffer code
- `pnpm test -- <focused runtime tests>` when protocol or client entity state changes
- `pnpm host:check` before choosing browser validation on an unfamiliar host
- smallest available visual lane that reaches the new draw path
- save screenshots to `/tmp` and inspect them before continuing
- `git diff --check`

On a headless host without browser WebGPU, prefer Deno WebGPU smokes and document the browser limitation.

## Done When

- [x] no standalone Steve/player WebGPU renderer remains
- [ ] entity buffers are produced through `MultiBufferSource` and `RenderType`
- [ ] player geometry comes from the ported `PlayerModel` / `ModelPart` path
- [ ] `LevelRenderer` owns entity render collection and returns generic entity draw batches
- [ ] remote multiplayer player snapshots render through `EntityRenderDispatcher`
- [ ] first visual smoke/probe screenshot confirms a nonblank neutral-pose player in world depth/fog/light context

## Next Step

Port `MultiBufferSource.BufferSource` and a renderer-owned `RenderBuffers` equivalent, then add a small unit test that proves batches start, switch, and flush by `RenderType` without drawing pixels yet.
