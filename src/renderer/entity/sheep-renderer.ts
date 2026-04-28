import { ResourceLocation } from "../../core/resource-location";
import type { MultiBufferSource } from "../multi-buffer-source";
import { ModelLayers } from "../model/geom/model-layers";
import { SheepFurModel } from "../model/sheep-fur-model";
import { SheepModel } from "../model/sheep-model";
import { RenderType } from "../render-type";
import { OverlayTexture } from "../texture/overlay-texture";
import type { PoseStack } from "../vertex/pose-stack";
import type { EntityRendererProvider } from "./entity-renderer-provider";
import { LivingEntityRenderer } from "./living-entity-renderer";
import type { RenderableSheep } from "./renderable-entity";

const SHEEP_LOCATION = new ResourceLocation("minecraft", "textures/entity/sheep/sheep.png");
const SHEEP_FUR_LOCATION = new ResourceLocation("minecraft", "textures/entity/sheep/sheep_fur.png");

export class SheepRenderer extends LivingEntityRenderer<RenderableSheep, SheepModel<RenderableSheep>> {
  private readonly furModel: SheepFurModel<RenderableSheep>;

  public constructor(context: EntityRendererProvider.Context) {
    super(context, new SheepModel(context.bakeLayer(ModelLayers.SHEEP)), 0.7);
    this.furModel = new SheepFurModel(context.bakeLayer(ModelLayers.SHEEP_FUR));
  }

  public getTextureLocation(entity: RenderableSheep): ResourceLocation {
    return entity.getTextureLocation();
  }

  protected override renderLayers(
    entity: RenderableSheep,
    _entityYaw: number,
    partialTicks: number,
    matrixStack: PoseStack,
    buffer: MultiBufferSource,
    packedLight: number,
    ageInTicks: number,
    netHeadYaw: number,
    headPitch: number,
  ): void {
    if (entity.isSheared()) {
      return;
    }

    this.model.copyPropertiesTo(this.furModel);
    this.furModel.prepareMobModel(entity, 0.0, 0.0, partialTicks);
    this.furModel.setupAnim(entity, 0.0, 0.0, ageInTicks, netHeadYaw, headPitch);
    const color = getSheepColor(entity.getColor());
    const vertexConsumer = buffer.getBuffer(RenderType.entityCutoutNoCull(SHEEP_FUR_LOCATION));
    this.furModel.renderToBuffer(
      matrixStack,
      vertexConsumer,
      packedLight,
      OverlayTexture.NO_OVERLAY,
      color[0],
      color[1],
      color[2],
      1.0,
    );
  }
}

function getSheepColor(colorId: number): readonly [number, number, number] {
  const color = SHEEP_COLORS[colorId] ?? SHEEP_COLORS[0]!;
  if (colorId === 0) {
    return [0.9019608, 0.9019608, 0.9019608];
  }
  return [color[0] * 0.75, color[1] * 0.75, color[2] * 0.75];
}

const SHEEP_COLORS: readonly (readonly [number, number, number])[] = [
  [0xf9 / 255, 0xff / 255, 0xfe / 255],
  [0xf9 / 255, 0x80 / 255, 0x1d / 255],
  [0xc7 / 255, 0x4e / 255, 0xbd / 255],
  [0x3a / 255, 0xb3 / 255, 0xda / 255],
  [0xfe / 255, 0xd8 / 255, 0x3d / 255],
  [0x80 / 255, 0xc7 / 255, 0x1f / 255],
  [0xf3 / 255, 0x8b / 255, 0xaa / 255],
  [0x47 / 255, 0x4f / 255, 0x52 / 255],
  [0x9d / 255, 0x9d / 255, 0x97 / 255],
  [0x16 / 255, 0x9c / 255, 0x9c / 255],
  [0x89 / 255, 0x32 / 255, 0xb8 / 255],
  [0x3c / 255, 0x44 / 255, 0xaa / 255],
  [0x83 / 255, 0x54 / 255, 0x32 / 255],
  [0x5e / 255, 0x7c / 255, 0x16 / 255],
  [0xb0 / 255, 0x2e / 255, 0x26 / 255],
  [0x1d / 255, 0x1d / 255, 0x21 / 255],
] as const;

export { SHEEP_FUR_LOCATION, SHEEP_LOCATION };
