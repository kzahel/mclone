import type { PoseStack } from "../vertex/pose-stack";
import type { VertexConsumer } from "../vertex/vertex-consumer";
import { EntityModel } from "./entity-model";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";

export interface RabbitModelEntity {
  readonly tickCount: number;
  getJumpCompletion(partialTick: number): number;
}

export class RabbitModel<T extends RabbitModelEntity> extends EntityModel<T> {
  private static readonly REAR_JUMP_ANGLE = 50.0;
  private static readonly FRONT_JUMP_ANGLE = -40.0;
  private readonly leftRearFoot: ModelPart;
  private readonly rightRearFoot: ModelPart;
  private readonly leftHaunch: ModelPart;
  private readonly rightHaunch: ModelPart;
  private readonly body: ModelPart;
  private readonly leftFrontLeg: ModelPart;
  private readonly rightFrontLeg: ModelPart;
  private readonly head: ModelPart;
  private readonly rightEar: ModelPart;
  private readonly leftEar: ModelPart;
  private readonly tail: ModelPart;
  private readonly nose: ModelPart;
  private jumpRotation = 0.0;

  public constructor(root: ModelPart) {
    super();
    this.leftRearFoot = root.getChild("left_hind_foot");
    this.rightRearFoot = root.getChild("right_hind_foot");
    this.leftHaunch = root.getChild("left_haunch");
    this.rightHaunch = root.getChild("right_haunch");
    this.body = root.getChild("body");
    this.leftFrontLeg = root.getChild("left_front_leg");
    this.rightFrontLeg = root.getChild("right_front_leg");
    this.head = root.getChild("head");
    this.rightEar = root.getChild("right_ear");
    this.leftEar = root.getChild("left_ear");
    this.tail = root.getChild("tail");
    this.nose = root.getChild("nose");
  }

  public static createBodyLayer(): LayerDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "left_hind_foot",
      CubeListBuilder.create().texOffs(26, 24).addBox(-1.0, 5.5, -3.7, 2.0, 1.0, 7.0),
      PartPose.offset(3.0, 17.5, 3.7),
    );
    root.addOrReplaceChild(
      "right_hind_foot",
      CubeListBuilder.create().texOffs(8, 24).addBox(-1.0, 5.5, -3.7, 2.0, 1.0, 7.0),
      PartPose.offset(-3.0, 17.5, 3.7),
    );
    root.addOrReplaceChild(
      "left_haunch",
      CubeListBuilder.create().texOffs(30, 15).addBox(-1.0, 0.0, 0.0, 2.0, 4.0, 5.0),
      PartPose.offsetAndRotation(3.0, 17.5, 3.7, -Math.PI / 9, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "right_haunch",
      CubeListBuilder.create().texOffs(16, 15).addBox(-1.0, 0.0, 0.0, 2.0, 4.0, 5.0),
      PartPose.offsetAndRotation(-3.0, 17.5, 3.7, -Math.PI / 9, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create().texOffs(0, 0).addBox(-3.0, -2.0, -10.0, 6.0, 5.0, 10.0),
      PartPose.offsetAndRotation(0.0, 19.0, 8.0, -Math.PI / 9, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "left_front_leg",
      CubeListBuilder.create().texOffs(8, 15).addBox(-1.0, 0.0, -1.0, 2.0, 7.0, 2.0),
      PartPose.offsetAndRotation(3.0, 17.0, -1.0, -Math.PI / 18, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "right_front_leg",
      CubeListBuilder.create().texOffs(0, 15).addBox(-1.0, 0.0, -1.0, 2.0, 7.0, 2.0),
      PartPose.offsetAndRotation(-3.0, 17.0, -1.0, -Math.PI / 18, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create().texOffs(32, 0).addBox(-2.5, -4.0, -5.0, 5.0, 4.0, 5.0),
      PartPose.offset(0.0, 16.0, -1.0),
    );
    root.addOrReplaceChild(
      "right_ear",
      CubeListBuilder.create().texOffs(52, 0).addBox(-2.5, -9.0, -1.0, 2.0, 5.0, 1.0),
      PartPose.offsetAndRotation(0.0, 16.0, -1.0, 0.0, -Math.PI / 12, 0.0),
    );
    root.addOrReplaceChild(
      "left_ear",
      CubeListBuilder.create().texOffs(58, 0).addBox(0.5, -9.0, -1.0, 2.0, 5.0, 1.0),
      PartPose.offsetAndRotation(0.0, 16.0, -1.0, 0.0, Math.PI / 12, 0.0),
    );
    root.addOrReplaceChild(
      "tail",
      CubeListBuilder.create().texOffs(52, 6).addBox(-1.5, -1.5, 0.0, 3.0, 3.0, 2.0),
      PartPose.offsetAndRotation(0.0, 20.0, 7.0, -0.3490659, 0.0, 0.0),
    );
    root.addOrReplaceChild(
      "nose",
      CubeListBuilder.create().texOffs(32, 9).addBox(-0.5, -2.5, -5.5, 1.0, 1.0, 1.0),
      PartPose.offset(0.0, 16.0, -1.0),
    );
    return LayerDefinition.create(meshDefinition, 64, 32);
  }

  public override renderToBuffer(
    poseStack: PoseStack,
    vertexConsumer: VertexConsumer,
    packedLight: number,
    packedOverlay: number,
    red: number,
    green: number,
    blue: number,
    alpha: number,
  ): void {
    if (this.young) {
      poseStack.pushPose();
      poseStack.scale(0.56666666, 0.56666666, 0.56666666);
      poseStack.translate(0.0, 1.375, 0.125);
      for (const part of [this.head, this.leftEar, this.rightEar, this.nose]) {
        part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
      }
      poseStack.popPose();
      poseStack.pushPose();
      poseStack.scale(0.4, 0.4, 0.4);
      poseStack.translate(0.0, 2.25, 0.0);
      for (const part of [
        this.leftRearFoot,
        this.rightRearFoot,
        this.leftHaunch,
        this.rightHaunch,
        this.body,
        this.leftFrontLeg,
        this.rightFrontLeg,
        this.tail,
      ]) {
        part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
      }
      poseStack.popPose();
      return;
    }

    poseStack.pushPose();
    poseStack.scale(0.6, 0.6, 0.6);
    poseStack.translate(0.0, 1.0, 0.0);
    for (const part of [
      this.leftRearFoot,
      this.rightRearFoot,
      this.leftHaunch,
      this.rightHaunch,
      this.body,
      this.leftFrontLeg,
      this.rightFrontLeg,
      this.head,
      this.rightEar,
      this.leftEar,
      this.tail,
      this.nose,
    ]) {
      part.render(poseStack, vertexConsumer, packedLight, packedOverlay, red, green, blue, alpha);
    }
    poseStack.popPose();
  }

  public override setupAnim(entity: T, _limbSwing: number, _limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    const partialTick = ageInTicks - entity.tickCount;
    this.nose.xRot = headPitch * (Math.PI / 180.0);
    this.head.xRot = headPitch * (Math.PI / 180.0);
    this.rightEar.xRot = headPitch * (Math.PI / 180.0);
    this.leftEar.xRot = headPitch * (Math.PI / 180.0);
    this.nose.yRot = netHeadYaw * (Math.PI / 180.0);
    this.head.yRot = netHeadYaw * (Math.PI / 180.0);
    this.rightEar.yRot = this.nose.yRot - (Math.PI / 12);
    this.leftEar.yRot = this.nose.yRot + (Math.PI / 12);
    this.jumpRotation = Math.sin(entity.getJumpCompletion(partialTick) * Math.PI);
    this.leftHaunch.xRot = ((this.jumpRotation * RabbitModel.REAR_JUMP_ANGLE) - 21.0) * (Math.PI / 180.0);
    this.rightHaunch.xRot = ((this.jumpRotation * RabbitModel.REAR_JUMP_ANGLE) - 21.0) * (Math.PI / 180.0);
    this.leftRearFoot.xRot = this.jumpRotation * RabbitModel.REAR_JUMP_ANGLE * (Math.PI / 180.0);
    this.rightRearFoot.xRot = this.jumpRotation * RabbitModel.REAR_JUMP_ANGLE * (Math.PI / 180.0);
    this.leftFrontLeg.xRot = ((this.jumpRotation * RabbitModel.FRONT_JUMP_ANGLE) - 11.0) * (Math.PI / 180.0);
    this.rightFrontLeg.xRot = ((this.jumpRotation * RabbitModel.FRONT_JUMP_ANGLE) - 11.0) * (Math.PI / 180.0);
  }

  public override prepareMobModel(entity: T, _limbSwing: number, _limbSwingAmount: number, partialTick: number): void {
    this.jumpRotation = Math.sin(entity.getJumpCompletion(partialTick) * Math.PI);
  }
}
