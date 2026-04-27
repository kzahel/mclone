import type { ResourceLocation } from "../../core/resource-location";
import { Vec3 } from "../../world/phys/vec3";
import type { MultiBufferSource } from "../multi-buffer-source";
import type { PoseStack } from "../vertex/pose-stack";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import type { RenderableEntity } from "./renderable-entity";

export abstract class EntityRenderer<T extends RenderableEntity> {
  protected readonly entityRenderDispatcher;
  protected shadowRadius = 0;
  protected shadowStrength = 1.0;

  protected constructor(context: EntityRendererProvider.Context) {
    this.entityRenderDispatcher = context.getEntityRenderDispatcher();
  }

  public getPackedLightCoords(_entity: T, _partialTicks: number): number {
    // EntityRender0: callers pass packed light directly; default to full-bright for isolated renderer tests.
    return 0x00f000f0;
  }

  public shouldRender(_entity: T, _camera: unknown, _camX: number, _camY: number, _camZ: number): boolean {
    // EntityRender0: entity frustum culling lands with LevelRenderer integration.
    return true;
  }

  public getRenderOffset(_entity: T, _partialTicks: number): Vec3 {
    return Vec3.ZERO;
  }

  public render(_entity: T, _entityYaw: number, _partialTicks: number, _matrixStack: PoseStack, _buffer: MultiBufferSource, _packedLight: number): void {
    // EntityRender0: name tags are deferred.
  }

  public abstract getTextureLocation(entity: T): ResourceLocation;
}
