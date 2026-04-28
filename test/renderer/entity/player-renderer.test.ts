import { describe, expect, test } from "vitest";
import { Vec3 } from "../../../src/world/phys/vec3";
import { LightTexture } from "../../../src/renderer/light-texture";
import { collectEntityRenderBatches } from "../../../src/renderer/entity/entity-batch-renderer";
import { EntityRenderDispatcher } from "../../../src/renderer/entity/entity-render-dispatcher";
import {
  DEFAULT_CHICKEN_TEXTURE,
  DEFAULT_COW_TEXTURE,
  DEFAULT_BROWN_MOOSHROOM_TEXTURE,
  DEFAULT_PIG_TEXTURE,
  DEFAULT_PLAYER_SKIN,
  DEFAULT_RABBIT_BROWN_TEXTURE,
  DEFAULT_RABBIT_EVIL_TEXTURE,
  DEFAULT_RED_MOOSHROOM_TEXTURE,
  DEFAULT_SHEEP_FUR_TEXTURE,
  DEFAULT_SHEEP_TEXTURE,
  DEFAULT_WOLF_ANGRY_TEXTURE,
  DEFAULT_WOLF_TEXTURE,
  DEFAULT_WOLF_TAME_TEXTURE,
  SnapshotRenderableChicken,
  SnapshotRenderableCow,
  SnapshotRenderableMooshroom,
  SnapshotRenderablePig,
  SnapshotRenderablePlayer,
  SnapshotRenderableRabbit,
  SnapshotRenderableSheep,
  SnapshotRenderableWolf,
  createRenderableEntity,
} from "../../../src/renderer/entity/renderable-entity";
import { MultiBufferSource } from "../../../src/renderer/multi-buffer-source";
import { RenderType } from "../../../src/renderer/render-type";
import { resolveEntityTexturePath } from "../../../src/renderer/texture/entity-texture-manager";
import { DefaultVertexFormat } from "../../../src/renderer/vertex/default-vertex-format";
import { BufferBuilder } from "../../../src/renderer/vertex/buffer-builder";
import { PoseStack } from "../../../src/renderer/vertex/pose-stack";
import type { ClientEntityPresentationState } from "../../../src/runtime/client/entity-interpolation-service";
import type { BlockAndTintGetter } from "../../../src/world/level/block-and-tint-getter";

function playerState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 7,
    uuid: "00000000-0000-0000-0000-000000000007",
    typeId: "minecraft:player",
    category: "misc" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 30, pitch: 12 },
    width: 0.6,
    height: 1.8,
    onGround: true,
    age: 42,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function cowState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 8,
    uuid: "00000000-0000-0000-0000-000000000008",
    typeId: "minecraft:cow",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.9,
    height: 1.4,
    onGround: true,
    age: 0,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function pigState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 9,
    uuid: "00000000-0000-0000-0000-000000000009",
    typeId: "minecraft:pig",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.9,
    height: 0.9,
    onGround: true,
    age: 0,
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: {},
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function mooshroomState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 12,
    uuid: "00000000-0000-0000-0000-000000000012",
    typeId: "minecraft:mooshroom",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.9,
    height: 1.4,
    onGround: true,
    age: 0,
    data: { Type: "red" },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function rabbitState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 13,
    uuid: "00000000-0000-0000-0000-000000000013",
    typeId: "minecraft:rabbit",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.4,
    height: 0.5,
    onGround: true,
    age: 0,
    data: { RabbitType: 0, JumpTicks: 0, JumpDuration: 0 },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function sheepState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 10,
    uuid: "00000000-0000-0000-0000-000000000010",
    typeId: "minecraft:sheep",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.9,
    height: 1.3,
    onGround: true,
    age: 0,
    data: { Color: 12, EatAnimationTick: 20 },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function wolfState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 14,
    uuid: "00000000-0000-0000-0000-000000000014",
    typeId: "minecraft:wolf",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.6,
    height: 0.85,
    onGround: true,
    age: 0,
    data: {
      Angry: false,
      CollarColor: 14,
      Health: 8,
      MaxHealth: 8,
      RemainingAngerTime: 0,
      Sitting: false,
      Tame: false,
      Wet: false,
    },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

function chickenState(overrides: Partial<ClientEntityPresentationState> = {}): ClientEntityPresentationState {
  const authoritative = {
    id: 11,
    uuid: "00000000-0000-0000-0000-000000000011",
    typeId: "minecraft:chicken",
    category: "creature" as const,
    chunkX: 0,
    chunkZ: 0,
    position: { x: 8, y: 65, z: 8 },
    rotation: { yaw: 45, pitch: 0 },
    width: 0.4,
    height: 0.7,
    onGround: true,
    age: 0,
    data: {
      Flap: 0.25,
      FlapSpeed: 0.5,
      OFlap: 0.0,
      OFlapSpeed: 0.25,
      Flapping: 1.0,
      EggLayTime: 9000,
      IsChickenJockey: false,
    },
  };
  return {
    entityId: authoritative.id,
    uuid: authoritative.uuid,
    typeId: authoritative.typeId,
    category: authoritative.category,
    chunkX: authoritative.chunkX,
    chunkZ: authoritative.chunkZ,
    width: authoritative.width,
    height: authoritative.height,
    onGround: authoritative.onGround,
    age: authoritative.age,
    data: authoritative.data,
    interpolatedPosition: authoritative.position,
    interpolatedRotation: authoritative.rotation,
    interpolationAlpha: 1,
    authoritative,
    aiAuthority: "host",
    ...overrides,
  };
}

class RecordingBufferSource extends MultiBufferSource.BufferSource {
  public readonly requested: RenderType[] = [];

  public constructor(private readonly sharedBuilder: BufferBuilder) {
    super(sharedBuilder, new Map());
  }

  public override getBuffer(renderType: RenderType) {
    this.requested.push(renderType);
    return super.getBuffer(renderType);
  }

  public pop() {
    return this.sharedBuilder.popNextBuffer();
  }
}

const FULL_BRIGHT_LEVEL = {
  getBrightness: () => 15,
} as unknown as BlockAndTintGetter;

describe("Entity renderer", () => {
  test("snapshot adapter creates renderable player state and ignores unsupported entities", () => {
    const state = playerState({
      data: {
        skinModel: "slim",
        skinTexture: "minecraft:textures/entity/custom_remote.png",
        crouching: true,
      },
    });
    const player = createRenderableEntity(state);
    expect(player).toBeInstanceOf(SnapshotRenderablePlayer);
    const renderable = player as SnapshotRenderablePlayer;
    expect(renderable.getModelName()).toBe("slim");
    expect(renderable.getSkinTextureLocation().toString()).toBe("minecraft:textures/entity/custom_remote.png");
    expect(renderable.isCrouching()).toBe(true);

    expect(createRenderableEntity({
      ...state,
      typeId: "minecraft:goat",
      authoritative: { ...state.authoritative, typeId: "minecraft:goat" },
    })).toBeUndefined();

    const cow = createRenderableEntity(cowState({
      data: {
        YBodyRot: 12,
        YBodyRotO: 8,
        YHeadRot: 20,
        YHeadRotO: 16,
        XRotO: -3,
      },
    }));
    expect(cow).toBeInstanceOf(SnapshotRenderableCow);
    expect(cow?.yBodyRot).toBe(12);
    expect(cow?.yBodyRotO).toBe(8);
    expect(cow?.yHeadRot).toBe(20);
    expect(cow?.yHeadRotO).toBe(16);
    expect(cow?.xRotO).toBe(-3);

    const pig = createRenderableEntity(pigState());
    expect(pig).toBeInstanceOf(SnapshotRenderablePig);

    const mooshroom = createRenderableEntity(mooshroomState({ data: { Type: "brown" } }));
    expect(mooshroom).toBeInstanceOf(SnapshotRenderableMooshroom);
    expect((mooshroom as SnapshotRenderableMooshroom).getMushroomType()).toBe("brown");
    expect((mooshroom as SnapshotRenderableMooshroom).getTextureLocation().toString()).toBe(DEFAULT_BROWN_MOOSHROOM_TEXTURE.toString());

    const rabbit = createRenderableEntity(rabbitState({ data: { RabbitType: 99, JumpTicks: 5, JumpDuration: 10 } }));
    expect(rabbit).toBeInstanceOf(SnapshotRenderableRabbit);
    expect((rabbit as SnapshotRenderableRabbit).getRabbitType()).toBe(99);
    expect((rabbit as SnapshotRenderableRabbit).getJumpCompletion(0)).toBe(0.5);
    expect((rabbit as SnapshotRenderableRabbit).getTextureLocation().toString()).toBe(DEFAULT_RABBIT_EVIL_TEXTURE.toString());

    const sheep = createRenderableEntity(sheepState());
    expect(sheep).toBeInstanceOf(SnapshotRenderableSheep);
    expect((sheep as SnapshotRenderableSheep).getColor()).toBe(12);
    expect((sheep as SnapshotRenderableSheep).getHeadEatPositionScale(0)).toBe(1);
    expect((sheep as SnapshotRenderableSheep).getHeadEatAngleScale(0)).toBeCloseTo(Math.PI / 5 + (0.21991149 * Math.sin(16 / 32 * 28.7)));

    const wolf = createRenderableEntity(wolfState({ data: { Tame: true, Health: 20, MaxHealth: 20 } }));
    expect(wolf).toBeInstanceOf(SnapshotRenderableWolf);
    expect((wolf as SnapshotRenderableWolf).isTame()).toBe(true);
    expect((wolf as SnapshotRenderableWolf).getTailAngle()).toBeCloseTo(0.55 * Math.PI);
    expect((wolf as SnapshotRenderableWolf).getTextureLocation().toString()).toBe(DEFAULT_WOLF_TAME_TEXTURE.toString());

    const chicken = createRenderableEntity(chickenState());
    expect(chicken).toBeInstanceOf(SnapshotRenderableChicken);
    expect((chicken as SnapshotRenderableChicken).getEggLayTime()).toBe(9000);
  });

  test("dispatcher renders a neutral remote player through entityTranslucent NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderablePlayer(playerState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityTranslucent(DEFAULT_PLAYER_SKIN)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(288);
    expect(drawState.indexCount()).toBe(432);
    expect(drawState.sequentialIndex()).toBe(false);
    expect(buffer.length).toBeGreaterThan(drawState.vertexBufferSize());
  });

  test("entity batch collection preserves render type and NEW_ENTITY draw state", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderablePlayer(playerState());

    const batches = collectEntityRenderBatches({
      level: FULL_BRIGHT_LEVEL,
      entities: [entity],
      cameraPosition: new Vec3(8, 65, 8),
      partialTick: 0,
      dispatcher,
    });

    expect(batches).toHaveLength(1);
    expect(batches[0]!.renderType).toBe(RenderType.entityTranslucent(DEFAULT_PLAYER_SKIN));
    expect(batches[0]!.drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(batches[0]!.drawState.vertexCount()).toBe(288);
    expect(batches[0]!.buffer.length).toBeGreaterThan(batches[0]!.drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral cow through entityCutoutNoCull NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableCow(cowState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_COW_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(216);
    expect(drawState.indexCount()).toBe(324);
    expect(drawState.sequentialIndex()).toBe(true);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("entity batch collection includes cow render batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableCow(cowState());

    const batches = collectEntityRenderBatches({
      level: FULL_BRIGHT_LEVEL,
      entities: [entity],
      cameraPosition: new Vec3(8, 65, 8),
      partialTick: 0,
      dispatcher,
    });

    expect(batches).toHaveLength(1);
    expect(batches[0]!.renderType).toBe(RenderType.entityCutoutNoCull(DEFAULT_COW_TEXTURE));
    expect(batches[0]!.drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(batches[0]!.drawState.vertexCount()).toBe(216);
    expect(batches[0]!.buffer.length).toBe(batches[0]!.drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral pig through entityCutoutNoCull NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderablePig(pigState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_PIG_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(168);
    expect(drawState.indexCount()).toBe(252);
    expect(drawState.sequentialIndex()).toBe(true);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral mooshroom through cow-shaped entityCutoutNoCull batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableMooshroom(mooshroomState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_RED_MOOSHROOM_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(216);
    expect(drawState.indexCount()).toBe(324);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral rabbit through entityCutoutNoCull NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableRabbit(rabbitState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_RABBIT_BROWN_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(288);
    expect(drawState.indexCount()).toBe(432);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral chicken through entityCutoutNoCull NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableChicken(chickenState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_CHICKEN_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(192);
    expect(drawState.indexCount()).toBe(288);
    expect(drawState.sequentialIndex()).toBe(true);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral wolf through entityCutoutNoCull NEW_ENTITY batches", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableWolf(wolfState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([RenderType.entityCutoutNoCull(DEFAULT_WOLF_TEXTURE)]));
    const { drawState, buffer } = bufferSource.pop();
    expect(drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(drawState.vertexCount()).toBe(264);
    expect(drawState.indexCount()).toBe(396);
    expect(buffer.length).toBe(drawState.vertexBufferSize());
  });

  test("dispatcher renders a neutral sheep with a colored fur layer", () => {
    const dispatcher = new EntityRenderDispatcher();
    const entity = new SnapshotRenderableSheep(sheepState());
    const builder = new BufferBuilder(256);
    const bufferSource = new RecordingBufferSource(builder);

    dispatcher.renderEntity(
      entity,
      new Vec3(8, 65, 8),
      new PoseStack(),
      bufferSource,
      LightTexture.pack(15, 15),
      0,
    );
    bufferSource.endBatch();

    expect(new Set(bufferSource.requested)).toEqual(new Set([
      RenderType.entityCutoutNoCull(DEFAULT_SHEEP_TEXTURE),
      RenderType.entityCutoutNoCull(DEFAULT_SHEEP_FUR_TEXTURE),
    ]));
    const base = bufferSource.pop();
    expect(base.drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(base.drawState.vertexCount()).toBe(144);
    expect(base.drawState.indexCount()).toBe(216);
    expect(base.buffer.length).toBe(base.drawState.vertexBufferSize());

    const fur = bufferSource.pop();
    expect(fur.drawState.format()).toBe(DefaultVertexFormat.NEW_ENTITY);
    expect(fur.drawState.vertexCount()).toBe(144);
    expect(fur.drawState.indexCount()).toBe(216);
    expect(fur.buffer.length).toBe(fur.drawState.vertexBufferSize());
  });

  test("entity texture paths resolve vanilla full texture locations directly", () => {
    expect(resolveEntityTexturePath(DEFAULT_PLAYER_SKIN)).toBe("assets/minecraft/textures/entity/steve.png");
    expect(resolveEntityTexturePath(DEFAULT_CHICKEN_TEXTURE)).toBe("assets/minecraft/textures/entity/chicken.png");
    expect(resolveEntityTexturePath(DEFAULT_COW_TEXTURE)).toBe("assets/minecraft/textures/entity/cow/cow.png");
    expect(resolveEntityTexturePath(DEFAULT_BROWN_MOOSHROOM_TEXTURE)).toBe("assets/minecraft/textures/entity/cow/brown_mooshroom.png");
    expect(resolveEntityTexturePath(DEFAULT_RED_MOOSHROOM_TEXTURE)).toBe("assets/minecraft/textures/entity/cow/red_mooshroom.png");
    expect(resolveEntityTexturePath(DEFAULT_PIG_TEXTURE)).toBe("assets/minecraft/textures/entity/pig/pig.png");
    expect(resolveEntityTexturePath(DEFAULT_RABBIT_BROWN_TEXTURE)).toBe("assets/minecraft/textures/entity/rabbit/brown.png");
    expect(resolveEntityTexturePath(DEFAULT_SHEEP_TEXTURE)).toBe("assets/minecraft/textures/entity/sheep/sheep.png");
    expect(resolveEntityTexturePath(DEFAULT_SHEEP_FUR_TEXTURE)).toBe("assets/minecraft/textures/entity/sheep/sheep_fur.png");
    expect(resolveEntityTexturePath(DEFAULT_WOLF_ANGRY_TEXTURE)).toBe("assets/minecraft/textures/entity/wolf/wolf_angry.png");
    expect(resolveEntityTexturePath(DEFAULT_WOLF_TEXTURE)).toBe("assets/minecraft/textures/entity/wolf/wolf.png");
    expect(resolveEntityTexturePath("minecraft:entity/custom_remote")).toBe("assets/minecraft/textures/entity/custom_remote.png");
  });
});
