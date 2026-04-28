import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { QuadrupedModel } from "./quadruped-model";
import type { SheepModelEntity } from "./sheep-model";

export class SheepFurModel<T extends SheepModelEntity> extends QuadrupedModel<T> {
  private headXRot = 0.0;

  public constructor(root: ModelPart) {
    super(root, false, 8.0, 4.0, 2.0, 2.0, 24);
  }

  public static createFurLayer(): LayerDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create()
        .texOffs(0, 0)
        .addBox(-3.0, -4.0, -4.0, 6.0, 6.0, 6.0, new CubeDeformation(0.6)),
      PartPose.offset(0.0, 6.0, -8.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create()
        .texOffs(28, 8)
        .addBox(-4.0, -10.0, -7.0, 8.0, 16.0, 6.0, new CubeDeformation(1.75)),
      PartPose.offsetAndRotation(0.0, 5.0, 2.0, Math.PI / 2, 0.0, 0.0),
    );
    const leg = CubeListBuilder.create()
      .texOffs(0, 16)
      .addBox(-2.0, 0.0, -2.0, 4.0, 6.0, 4.0, new CubeDeformation(0.5));
    root.addOrReplaceChild("right_hind_leg", leg, PartPose.offset(-3.0, 12.0, 7.0));
    root.addOrReplaceChild("left_hind_leg", leg, PartPose.offset(3.0, 12.0, 7.0));
    root.addOrReplaceChild("right_front_leg", leg, PartPose.offset(-3.0, 12.0, -5.0));
    root.addOrReplaceChild("left_front_leg", leg, PartPose.offset(3.0, 12.0, -5.0));
    return LayerDefinition.create(meshDefinition, 64, 32);
  }

  public override prepareMobModel(entity: T, limbSwing: number, limbSwingAmount: number, partialTick: number): void {
    super.prepareMobModel(entity, limbSwing, limbSwingAmount, partialTick);
    this.head.y = 6.0 + (entity.getHeadEatPositionScale(partialTick) * 9.0);
    this.headXRot = entity.getHeadEatAngleScale(partialTick);
  }

  public override setupAnim(entity: T, limbSwing: number, limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    super.setupAnim(entity, limbSwing, limbSwingAmount, ageInTicks, netHeadYaw, headPitch);
    this.head.xRot = this.headXRot;
  }
}
