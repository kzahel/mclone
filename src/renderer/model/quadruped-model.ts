import { RenderType } from "../render-type";
import { AgeableListModel } from "./ageable-list-model";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";

export class QuadrupedModel<T = unknown> extends AgeableListModel<T> {
  protected readonly head: ModelPart;
  protected readonly body: ModelPart;
  protected readonly rightHindLeg: ModelPart;
  protected readonly leftHindLeg: ModelPart;
  protected readonly rightFrontLeg: ModelPart;
  protected readonly leftFrontLeg: ModelPart;

  protected constructor(
    root: ModelPart,
    scaleHead: boolean,
    babyYHeadOffset: number,
    babyZHeadOffset: number,
    babyHeadScale: number,
    babyBodyScale: number,
    bodyYOffset: number,
  ) {
    super(RenderType.entityCutoutNoCull, scaleHead, babyYHeadOffset, babyZHeadOffset, babyHeadScale, babyBodyScale, bodyYOffset);
    this.head = root.getChild("head");
    this.body = root.getChild("body");
    this.rightHindLeg = root.getChild("right_hind_leg");
    this.leftHindLeg = root.getChild("left_hind_leg");
    this.rightFrontLeg = root.getChild("right_front_leg");
    this.leftFrontLeg = root.getChild("left_front_leg");
  }

  public static createBodyMesh(legSize: number, cubeDeformation: CubeDeformation): MeshDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create().texOffs(0, 0).addBox(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0, cubeDeformation),
      PartPose.offset(0.0, 18 - legSize, -6.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create().texOffs(28, 8).addBox(-5.0, -10.0, -7.0, 10.0, 16.0, 8.0, cubeDeformation),
      PartPose.offsetAndRotation(0.0, 17 - legSize, 2.0, Math.PI / 2, 0.0, 0.0),
    );
    const leg = CubeListBuilder.create().texOffs(0, 16).addBox(-2.0, 0.0, -2.0, 4.0, legSize, 4.0, cubeDeformation);
    root.addOrReplaceChild("right_hind_leg", leg, PartPose.offset(-3.0, 24 - legSize, 7.0));
    root.addOrReplaceChild("left_hind_leg", leg, PartPose.offset(3.0, 24 - legSize, 7.0));
    root.addOrReplaceChild("right_front_leg", leg, PartPose.offset(-3.0, 24 - legSize, -5.0));
    root.addOrReplaceChild("left_front_leg", leg, PartPose.offset(3.0, 24 - legSize, -5.0));
    return meshDefinition;
  }

  protected override headParts(): Iterable<ModelPart> {
    return [this.head];
  }

  protected override bodyParts(): Iterable<ModelPart> {
    return [this.body, this.rightHindLeg, this.leftHindLeg, this.rightFrontLeg, this.leftFrontLeg];
  }

  public override setupAnim(_entity: T, limbSwing: number, limbSwingAmount: number, _ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    this.head.xRot = headPitch * (Math.PI / 180.0);
    this.head.yRot = netHeadYaw * (Math.PI / 180.0);
    this.rightHindLeg.xRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
    this.leftHindLeg.xRot = Math.cos(limbSwing * 0.6662 + Math.PI) * 1.4 * limbSwingAmount;
    this.rightFrontLeg.xRot = Math.cos(limbSwing * 0.6662 + Math.PI) * 1.4 * limbSwingAmount;
    this.leftFrontLeg.xRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
  }
}
