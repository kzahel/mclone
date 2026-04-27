import { BlockPos } from "../../core/block-pos";
import type { ClientEntityPresentationState } from "../../runtime/client/entity-interpolation-service";
import { floor } from "../../util/mth";
import type { BlockAndTintGetter } from "../../world/level/block-and-tint-getter";
import { LightLayer } from "../../world/level/light-layer";
import type { Vec3 } from "../../world/phys/vec3";
import { LightTexture } from "../light-texture";
import { MultiBufferSource } from "../multi-buffer-source";
import { RenderType } from "../render-type";
import { BufferBuilder, type BufferBuilderDrawState } from "../vertex/buffer-builder";
import { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { EntityRenderDispatcher } from "./entity-render-dispatcher";
import { createRenderableEntities, type RenderableEntity } from "./renderable-entity";

export interface EntityRenderBatch {
  readonly renderType: RenderType;
  readonly drawState: BufferBuilderDrawState;
  readonly buffer: Uint8Array;
}

export interface CollectEntityRenderBatchesOptions {
  readonly level: BlockAndTintGetter;
  readonly entities: readonly RenderableEntity[];
  readonly cameraPosition: Vec3;
  readonly partialTick: number;
  readonly dispatcher?: EntityRenderDispatcher;
}

export interface CollectPresentationEntityRenderBatchesOptions {
  readonly level: BlockAndTintGetter;
  readonly presentation: readonly ClientEntityPresentationState[];
  readonly cameraPosition: Vec3;
  readonly partialTick: number;
  readonly dispatcher?: EntityRenderDispatcher;
}

class EntityBatchBufferSource extends MultiBufferSource.BufferSource {
  private readonly completedRenderTypes: RenderType[] = [];

  public constructor(private readonly sharedBuilder: BufferBuilder) {
    super(sharedBuilder, new Map());
  }

  public override getBuffer(renderType: RenderType): VertexConsumer {
    return super.getBuffer(renderType);
  }

  public override endBatch(): void;
  public override endBatch(renderType: RenderType): void;
  public override endBatch(renderType?: RenderType): void {
    if (renderType !== undefined) {
      this.endRecordedBatch(renderType);
      return;
    }

    if (this.lastState !== undefined && !this.fixedBuffers.has(this.lastState)) {
      this.endRecordedBatch(this.lastState);
    }

    for (const fixedRenderType of this.fixedBuffers.keys()) {
      this.endRecordedBatch(fixedRenderType);
    }
  }

  private endRecordedBatch(renderType: RenderType): void {
    const builder = this.fixedBuffers.get(renderType) ?? this.builder;
    const isLastState = this.lastState === renderType.asOptional();
    const willEnd = (isLastState || builder !== this.builder) && this.startedBuffers.has(builder);
    super.endBatch(renderType);
    if (willEnd) {
      this.completedRenderTypes.push(renderType);
    }
  }

  public popCompletedBatches(): readonly EntityRenderBatch[] {
    return this.completedRenderTypes.map((renderType) => {
      const { drawState, buffer } = this.sharedBuilder.popNextBuffer();
      return {
        renderType,
        drawState,
        buffer,
      };
    });
  }
}

export function collectPresentationEntityRenderBatches(
  options: CollectPresentationEntityRenderBatchesOptions,
): readonly EntityRenderBatch[] {
  if (options.presentation.length === 0) {
    return [];
  }

  return collectEntityRenderBatches({
    ...options,
    entities: createRenderableEntities(options.presentation),
  });
}

export function collectEntityRenderBatches(options: CollectEntityRenderBatchesOptions): readonly EntityRenderBatch[] {
  if (options.entities.length === 0) {
    return [];
  }

  const dispatcher = options.dispatcher ?? new EntityRenderDispatcher();
  const builder = new BufferBuilder(256);
  const bufferSource = new EntityBatchBufferSource(builder);
  // WebGPU: emit camera-relative entity vertices and let the shared model-view uniform apply the camera rotation.
  const poseStack = new PoseStack();
  for (const entity of options.entities) {
    dispatcher.renderEntity(
      entity,
      options.cameraPosition,
      poseStack,
      bufferSource,
      getPackedLightCoords(options.level, entity),
      options.partialTick,
    );
  }
  bufferSource.endBatch();

  return bufferSource.popCompletedBatches();
}

function getPackedLightCoords(level: BlockAndTintGetter, entity: RenderableEntity): number {
  const pos = getLightProbeBlockPos(entity);
  return LightTexture.pack(
    level.getBrightness(LightLayer.BLOCK, pos),
    level.getBrightness(LightLayer.SKY, pos),
  );
}

function getLightProbeBlockPos(entity: RenderableEntity): BlockPos {
  return new BlockPos(
    floor(entity.getX()),
    floor(entity.getY() + (entity.getBbHeight() * 0.85)),
    floor(entity.getZ()),
  );
}
