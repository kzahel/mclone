import { AgeableListModel } from "./ageable-list-model";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";

export interface WolfModelEntity {
  getBodyRollAngle(partialTicks: number, offset: number): number;
  getHeadRollAngle(partialTicks: number): number;
  isAngry(): boolean;
  isInSittingPose(): boolean;
}

export class WolfModel<T extends WolfModelEntity> extends AgeableListModel<T> {
  private readonly head: ModelPart;
  private readonly realHead: ModelPart;
  private readonly body: ModelPart;
  private readonly rightHindLeg: ModelPart;
  private readonly leftHindLeg: ModelPart;
  private readonly rightFrontLeg: ModelPart;
  private readonly leftFrontLeg: ModelPart;
  private readonly tail: ModelPart;
  private readonly realTail: ModelPart;
  private readonly upperBody: ModelPart;

  public constructor(root: ModelPart) {
    super();
    this.head = root.getChild("head");
    this.realHead = this.head.getChild("real_head");
    this.body = root.getChild("body");
    this.upperBody = root.getChild("upper_body");
    this.rightHindLeg = root.getChild("right_hind_leg");
    this.leftHindLeg = root.getChild("left_hind_leg");
    this.rightFrontLeg = root.getChild("right_front_leg");
    this.leftFrontLeg = root.getChild("left_front_leg");
    this.tail = root.getChild("tail");
    this.realTail = this.tail.getChild("real_tail");
  }

  public static createBodyLayer(): LayerDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    const head = root.addOrReplaceChild("head", CubeListBuilder.create(), PartPose.offset(-1.0, 13.5, -7.0));
    head.addOrReplaceChild(
      "real_head",
      CubeListBuilder.create()
        .texOffs(0, 0)
        .addBox(-2.0, -3.0, -2.0, 6.0, 6.0, 4.0)
        .texOffs(16, 14)
        .addBox(-2.0, -5.0, 0.0, 2.0, 2.0, 1.0)
        .texOffs(16, 14)
        .addBox(2.0, -5.0, 0.0, 2.0, 2.0, 1.0)
        .texOffs(0, 10)
        .addBox(-0.5, 0.0, -5.0, 3.0, 3.0, 4.0),
      PartPose.ZERO,
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create().texOffs(18, 14).addBox(-3.0, -2.0, -3.0, 6.0, 9.0, 6.0),
      PartPose.offsetAndRotation(0.0, 14.0, 2.0, Math.PI / 2, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "upper_body",
      CubeListBuilder.create().texOffs(21, 0).addBox(-3.0, -3.0, -3.0, 8.0, 6.0, 7.0),
      PartPose.offsetAndRotation(-1.0, 14.0, -3.0, Math.PI / 2, 0.0, 0.0),
    );
    const leg = CubeListBuilder.create().texOffs(0, 18).addBox(0.0, 0.0, -1.0, 2.0, 8.0, 2.0);
    root.addOrReplaceChild("right_hind_leg", leg, PartPose.offset(-2.5, 16.0, 7.0));
    root.addOrReplaceChild("left_hind_leg", leg, PartPose.offset(0.5, 16.0, 7.0));
    root.addOrReplaceChild("right_front_leg", leg, PartPose.offset(-2.5, 16.0, -4.0));
    root.addOrReplaceChild("left_front_leg", leg, PartPose.offset(0.5, 16.0, -4.0));
    const tail = root.addOrReplaceChild(
      "tail",
      CubeListBuilder.create(),
      PartPose.offsetAndRotation(-1.0, 12.0, 8.0, Math.PI / 5, 0.0, 0.0),
    );
    tail.addOrReplaceChild(
      "real_tail",
      CubeListBuilder.create().texOffs(9, 18).addBox(0.0, 0.0, -1.0, 2.0, 8.0, 2.0),
      PartPose.ZERO,
    );
    return LayerDefinition.create(meshDefinition, 64, 32);
  }

  protected override headParts(): Iterable<ModelPart> {
    return [this.head];
  }

  protected override bodyParts(): Iterable<ModelPart> {
    return [this.body, this.rightHindLeg, this.leftHindLeg, this.rightFrontLeg, this.leftFrontLeg, this.tail, this.upperBody];
  }

  public override prepareMobModel(entity: T, limbSwing: number, limbSwingAmount: number, partialTick: number): void {
    if (entity.isAngry()) {
      this.tail.yRot = 0.0;
    } else {
      this.tail.yRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
    }

    if (entity.isInSittingPose()) {
      this.upperBody.setPos(-1.0, 16.0, -3.0);
      this.upperBody.xRot = Math.PI * 2.0 / 5.0;
      this.upperBody.yRot = 0.0;
      this.body.setPos(0.0, 18.0, 0.0);
      this.body.xRot = Math.PI / 4;
      this.tail.setPos(-1.0, 21.0, 6.0);
      this.rightHindLeg.setPos(-2.5, 22.7, 2.0);
      this.rightHindLeg.xRot = Math.PI * 3.0 / 2.0;
      this.leftHindLeg.setPos(0.5, 22.7, 2.0);
      this.leftHindLeg.xRot = Math.PI * 3.0 / 2.0;
      this.rightFrontLeg.xRot = 5.811947;
      this.rightFrontLeg.setPos(-2.49, 17.0, -4.0);
      this.leftFrontLeg.xRot = 5.811947;
      this.leftFrontLeg.setPos(0.51, 17.0, -4.0);
    } else {
      this.body.setPos(0.0, 14.0, 2.0);
      this.body.xRot = Math.PI / 2;
      this.upperBody.setPos(-1.0, 14.0, -3.0);
      this.upperBody.xRot = this.body.xRot;
      this.tail.setPos(-1.0, 12.0, 8.0);
      this.rightHindLeg.setPos(-2.5, 16.0, 7.0);
      this.leftHindLeg.setPos(0.5, 16.0, 7.0);
      this.rightFrontLeg.setPos(-2.5, 16.0, -4.0);
      this.leftFrontLeg.setPos(0.5, 16.0, -4.0);
      this.rightHindLeg.xRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
      this.leftHindLeg.xRot = Math.cos((limbSwing * 0.6662) + Math.PI) * 1.4 * limbSwingAmount;
      this.rightFrontLeg.xRot = Math.cos((limbSwing * 0.6662) + Math.PI) * 1.4 * limbSwingAmount;
      this.leftFrontLeg.xRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
    }

    this.realHead.zRot = entity.getHeadRollAngle(partialTick) + entity.getBodyRollAngle(partialTick, 0.0);
    this.upperBody.zRot = entity.getBodyRollAngle(partialTick, -0.08);
    this.body.zRot = entity.getBodyRollAngle(partialTick, -0.16);
    this.realTail.zRot = entity.getBodyRollAngle(partialTick, -0.2);
  }

  public override setupAnim(_entity: T, _limbSwing: number, _limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    this.head.xRot = headPitch * (Math.PI / 180.0);
    this.head.yRot = netHeadYaw * (Math.PI / 180.0);
    this.tail.xRot = ageInTicks;
  }
}
