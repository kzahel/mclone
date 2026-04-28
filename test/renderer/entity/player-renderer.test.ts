import { describe, expect, test } from "vitest";
import { Vec3 } from "../../../src/world/phys/vec3";
import { LightTexture } from "../../../src/renderer/light-texture";
import { collectEntityRenderBatches } from "../../../src/renderer/entity/entity-batch-renderer";
import { EntityRenderDispatcher } from "../../../src/renderer/entity/entity-render-dispatcher";
import {
  DEFAULT_COW_TEXTURE,
  DEFAULT_PLAYER_SKIN,
  SnapshotRenderableCow,
  SnapshotRenderablePlayer,
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

describe("Player entity renderer", () => {
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
      typeId: "minecraft:sheep",
      authoritative: { ...state.authoritative, typeId: "minecraft:sheep" },
    })).toBeUndefined();

    const cow = createRenderableEntity(cowState());
    expect(cow).toBeInstanceOf(SnapshotRenderableCow);
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

  test("entity texture paths resolve vanilla full texture locations directly", () => {
    expect(resolveEntityTexturePath(DEFAULT_PLAYER_SKIN)).toBe("assets/minecraft/textures/entity/steve.png");
    expect(resolveEntityTexturePath(DEFAULT_COW_TEXTURE)).toBe("assets/minecraft/textures/entity/cow/cow.png");
    expect(resolveEntityTexturePath("minecraft:entity/custom_remote")).toBe("assets/minecraft/textures/entity/custom_remote.png");
  });
});
