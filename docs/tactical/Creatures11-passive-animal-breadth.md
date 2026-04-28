# Creatures11 - Passive animal breadth

Status: mooshroom, rabbit, and wolf baseline breadth landed.

This slice expands visible passive-animal breadth without adding another interaction-heavy gameplay system. The goal is to make more animals already present in vanilla overworld spawn tables first-class generated/rendered mobs through the existing host-owned entity lifecycle, shared movement stack, and entity renderer path.

## Vanilla Sources Read

| Concern | Source |
|---|---|
| Mooshroom behavior, mycelium preference, type data, and shearing/interactions | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/MushroomCow.java` |
| Mooshroom renderer and mushroom block layer | `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/MushroomCowRenderer.java`, `layers/MushroomCowMushroomLayer.java` |
| Rabbit behavior, type data, jump state, spawn colors, garden raid, and goals | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Rabbit.java` |
| Rabbit model and renderer textures | `reference/minecraft-1.17.1/src/net/minecraft/client/model/RabbitModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/RabbitRenderer.java` |
| Wolf behavior, tame/angry/sitting/wet state, passive goals, and target goals | `reference/minecraft-1.17.1/src/net/minecraft/world/entity/animal/Wolf.java` |
| Wolf model, renderer, and collar layer | `reference/minecraft-1.17.1/src/net/minecraft/client/model/WolfModel.java`, `reference/minecraft-1.17.1/src/net/minecraft/client/renderer/entity/WolfRenderer.java`, `layers/WolfCollarLayer.java` |
| Model layer registration | `reference/minecraft-1.17.1/src/net/minecraft/client/model/geom/ModelLayers.java`, `LayerDefinitions.java` |

## Vanilla Model

- Mooshrooms inherit cow goals and attributes, render with the cow model on `ModelLayers.MOOSHROOM`, choose red/brown textures from `MushroomCow.Type`, prefer mycelium for walk target value, and add three rendered mushroom blocks as an entity layer.
- Rabbits define `RabbitType`, jump timing fields, custom jump/move controls, garden-raiding behavior, avoid-player/wolf/monster goals, and type-specific textures including Toast and killer bunny variants.
- Wolves define tame/angry/sitting/interested/wet/shaking state, use the wolf model tail/head/body roll formulas, choose normal/tame/angry textures, and render a dyed collar layer when tame.
- Wolf AI in vanilla is deeper than passive wandering: sitting, owner following, melee attack, prey targeting, anger, taming, and breeding all sit around the shared stroll/look goals.

## Landed Scope

- Added protocol/render IDs for `minecraft:mooshroom`, `minecraft:rabbit`, and `minecraft:wolf`.
- Registered vanilla-shaped attributes and baseline snapshot data:
  - mooshroom: cow dimensions/attributes, `Type=red`
  - rabbit: max health `3.0`, movement speed `0.3`, `RabbitType`, jump timing placeholders
  - wolf: max health `8.0`, movement speed `0.3`, tame/angry/sitting/wet/roll placeholder data
- Added generated-mob baseline goals from the vanilla priority layout where current infrastructure exists:
  - mooshroom: cow stroll/look/random-look priorities
  - rabbit: `WaterAvoidingRandomStrollGoal` at priority `6` with speed `0.6`, `LookAtPlayerGoal` at priority `11` with range `10`
  - wolf: `WaterAvoidingRandomStrollGoal` at priority `8`, `LookAtPlayerGoal` and `RandomLookAroundGoal` at priority `10`
- Ported `RabbitModel` geometry, adult/baby render scaling, and jump animation formulas.
- Ported `WolfModel` geometry, sitting pose, leg/tail animation, and head/body/tail roll formulas.
- Added mooshroom, rabbit, and wolf renderers and texture preloading for their vanilla texture families.
- Expanded the small-island starter animal set so these species are visible immediately through the same authoritative host publication path.
- Added renderer, entity-data, movement, island-spawn, host-publication, and Deno smoke coverage.

## Runtime Divergence

- Mooshroom mushroom block rendering is deferred. Vanilla `MushroomCowMushroomLayer` needs entity render layers to access the block renderer and block atlas; current entity-render context only owns model/texture data.
- Rabbit jumping still renders from snapshot jump fields but generated rabbits do not yet run the vanilla `RabbitJumpControl`, `RabbitMoveControl`, landing delay, avoid goals, killer-bunny attacks, or carrot garden raiding.
- Rabbit biome-specific spawn type selection is deferred. Runtime-generated rabbits currently default to brown unless explicit snapshot data supplies another `RabbitType`.
- Wolf tame/sitting/angry/wet/collar state is represented in snapshot data and render formulas, but the gameplay systems that mutate those states are deferred.
- Wolf collar rendering is deferred with the same render-layer gap as mooshroom mushroom blocks.
- Sounds, particles, damage/death, loot, breeding, taming, owner following, target selectors, and item interactions remain out of scope.

## Validation

```bash
pnpm test -- test/worldgen/levelgen/demo-world-generators.test.ts test/renderer/entity/player-renderer.test.ts test/world/entity/cow-ai.test.ts test/runtime/generated-world-host-entities.test.ts
pnpm --silent typecheck
npx -y deno@2.7.13 run --unstable-webgpu --allow-read=. --allow-write=/tmp ./scripts/deno-entity-render-smoke.ts
```

The Deno entity smoke writes `/tmp/mclone-deno-entity-render-smoke.png`.

## Next Step

Continue breadth with the next spawn-table families: horse/donkey for plains/savanna and goat/llama for mountain biomes. That should stay baseline-first: vanilla dimensions, attributes, model/renderer/texture hydration, generated snapshot data needed for neutral rendering, and small-island/debug visibility before riding, chests, carpets, spitting, ramming, or taming systems.
