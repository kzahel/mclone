import type { ResourceLocation } from "../../core/resource-location";
import { RenderType } from "../render-type";
import { PoseStack } from "../vertex/pose-stack";
import { AgeableListModel } from "./ageable-list-model";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { MeshDefinition } from "./geom/builders/mesh-definition";

export type HumanoidArm = "left" | "right";

export class HumanoidModel<T = unknown> extends AgeableListModel<T> {
  public readonly head: ModelPart;
  public readonly hat: ModelPart;
  public readonly body: ModelPart;
  public readonly rightArm: ModelPart;
  public readonly leftArm: ModelPart;
  public readonly rightLeg: ModelPart;
  public readonly leftLeg: ModelPart;
  public crouching = false;
  public swimAmount = 0;

  public constructor(root: ModelPart, renderType: (location: ResourceLocation) => RenderType = RenderType.entityCutoutNoCull) {
    super(renderType, true, 16.0, 0.0, 2.0, 2.0, 24.0);
    this.head = root.getChild("head");
    this.hat = root.getChild("hat");
    this.body = root.getChild("body");
    this.rightArm = root.getChild("right_arm");
    this.leftArm = root.getChild("left_arm");
    this.rightLeg = root.getChild("right_leg");
    this.leftLeg = root.getChild("left_leg");
  }

  public static createMesh(cubeDeformation: CubeDeformation, yOffsetOrSlim: number | boolean): MeshDefinition {
    const yOffset = typeof yOffsetOrSlim === "number" ? yOffsetOrSlim : 0.0;
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create().texOffs(0, 0).addBox(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, cubeDeformation),
      PartPose.offset(0.0, 0.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "hat",
      CubeListBuilder.create().texOffs(32, 0).addBox(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, cubeDeformation.extend(0.5)),
      PartPose.offset(0.0, 0.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create().texOffs(16, 16).addBox(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(0.0, 0.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "right_arm",
      CubeListBuilder.create().texOffs(40, 16).addBox(-3.0, -2.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(-5.0, 2.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "left_arm",
      CubeListBuilder.create().texOffs(40, 16).mirror().addBox(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(5.0, 2.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "right_leg",
      CubeListBuilder.create().texOffs(0, 16).addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(-1.9, 12.0 + yOffset, 0.0),
    );
    root.addOrReplaceChild(
      "left_leg",
      CubeListBuilder.create().texOffs(0, 16).mirror().addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(1.9, 12.0 + yOffset, 0.0),
    );
    return meshDefinition;
  }

  protected override headParts(): Iterable<ModelPart> {
    return [this.head];
  }

  protected override bodyParts(): Iterable<ModelPart> {
    return [this.body, this.rightArm, this.leftArm, this.rightLeg, this.leftLeg, this.hat];
  }

  public override setupAnim(_entity: T, _limbSwing: number, _limbSwingAmount: number, _ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    // EntityRender0: neutral-pose MVP keeps head pitch/yaw and crouch offsets, deferring limb/arm/swim/attack animation.
    this.head.yRot = netHeadYaw * (Math.PI / 180.0);
    this.head.xRot = headPitch * (Math.PI / 180.0);
    this.body.yRot = 0.0;
    this.body.xRot = this.crouching ? 0.5 : 0.0;
    this.rightArm.setPos(-5.0, this.crouching ? 5.2 : 2.0, 0.0);
    this.leftArm.setPos(5.0, this.crouching ? 5.2 : 2.0, 0.0);
    this.rightArm.setRotation(0.0, 0.0, 0.0);
    this.leftArm.setRotation(0.0, 0.0, 0.0);
    this.rightLeg.setPos(-1.9, this.crouching ? 12.2 : 12.0, this.crouching ? 4.0 : 0.1);
    this.leftLeg.setPos(1.9, this.crouching ? 12.2 : 12.0, this.crouching ? 4.0 : 0.1);
    this.rightLeg.setRotation(0.0, 0.0, 0.0);
    this.leftLeg.setRotation(0.0, 0.0, 0.0);
    this.head.y = this.crouching ? 4.2 : 0.0;
    this.body.y = this.crouching ? 3.2 : 0.0;
    this.hat.copyFrom(this.head);
  }

  public override copyPropertiesTo(model: HumanoidModel<T>): void {
    super.copyPropertiesTo(model);
    model.crouching = this.crouching;
    model.head.copyFrom(this.head);
    model.hat.copyFrom(this.hat);
    model.body.copyFrom(this.body);
    model.rightArm.copyFrom(this.rightArm);
    model.leftArm.copyFrom(this.leftArm);
    model.rightLeg.copyFrom(this.rightLeg);
    model.leftLeg.copyFrom(this.leftLeg);
  }

  public setAllVisible(visible: boolean): void {
    this.head.visible = visible;
    this.hat.visible = visible;
    this.body.visible = visible;
    this.rightArm.visible = visible;
    this.leftArm.visible = visible;
    this.rightLeg.visible = visible;
    this.leftLeg.visible = visible;
  }

  public translateToHand(side: HumanoidArm, poseStack: PoseStack): void {
    this.getArm(side).translateAndRotate(poseStack);
  }

  protected getArm(side: HumanoidArm): ModelPart {
    return side === "left" ? this.leftArm : this.rightArm;
  }

  public getHead(): ModelPart {
    return this.head;
  }
}
