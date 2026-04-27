import type { VertexConsumer } from "../vertex/vertex-consumer";
import type { PoseStack } from "../vertex/pose-stack";
import { RenderType } from "../render-type";
import { HumanoidModel, type HumanoidArm } from "./humanoid-model";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import type { MeshDefinition } from "./geom/builders/mesh-definition";

export interface PlayerModelEntity {
  isCrouching(): boolean;
  hasChestEquipment(): boolean;
}

export class PlayerModel<T extends PlayerModelEntity = PlayerModelEntity> extends HumanoidModel<T> {
  public readonly leftSleeve: ModelPart;
  public readonly rightSleeve: ModelPart;
  public readonly leftPants: ModelPart;
  public readonly rightPants: ModelPart;
  public readonly jacket: ModelPart;
  private readonly cloak: ModelPart;
  private readonly ear: ModelPart;
  private readonly partsValue: readonly ModelPart[];

  public constructor(root: ModelPart, private readonly slim: boolean) {
    super(root, RenderType.entityTranslucent);
    this.ear = root.getChild("ear");
    this.cloak = root.getChild("cloak");
    this.leftSleeve = root.getChild("left_sleeve");
    this.rightSleeve = root.getChild("right_sleeve");
    this.leftPants = root.getChild("left_pants");
    this.rightPants = root.getChild("right_pants");
    this.jacket = root.getChild("jacket");
    this.partsValue = root.getAllParts().filter((part) => !part.isEmpty());
  }

  public static override createMesh(cubeDeformation: CubeDeformation, slimOrYOffset: number | boolean): MeshDefinition {
    const slim = slimOrYOffset === true;
    const meshDefinition = HumanoidModel.createMesh(cubeDeformation, 0.0);
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "ear",
      CubeListBuilder.create().texOffs(24, 0).addBox(-3.0, -6.0, -1.0, 6.0, 6.0, 1.0, cubeDeformation),
      PartPose.ZERO,
    );
    root.addOrReplaceChild(
      "cloak",
      CubeListBuilder.create().texOffs(0, 0).addBox(-5.0, 0.0, -1.0, 10.0, 16.0, 1.0, cubeDeformation, 1.0, 0.5),
      PartPose.offset(0.0, 0.0, 0.0),
    );
    if (slim) {
      root.addOrReplaceChild(
        "left_arm",
        CubeListBuilder.create().texOffs(32, 48).addBox(-1.0, -2.0, -2.0, 3.0, 12.0, 4.0, cubeDeformation),
        PartPose.offset(5.0, 2.5, 0.0),
      );
      root.addOrReplaceChild(
        "right_arm",
        CubeListBuilder.create().texOffs(40, 16).addBox(-2.0, -2.0, -2.0, 3.0, 12.0, 4.0, cubeDeformation),
        PartPose.offset(-5.0, 2.5, 0.0),
      );
      root.addOrReplaceChild(
        "left_sleeve",
        CubeListBuilder.create().texOffs(48, 48).addBox(-1.0, -2.0, -2.0, 3.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
        PartPose.offset(5.0, 2.5, 0.0),
      );
      root.addOrReplaceChild(
        "right_sleeve",
        CubeListBuilder.create().texOffs(40, 32).addBox(-2.0, -2.0, -2.0, 3.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
        PartPose.offset(-5.0, 2.5, 0.0),
      );
    } else {
      root.addOrReplaceChild(
        "left_arm",
        CubeListBuilder.create().texOffs(32, 48).addBox(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
        PartPose.offset(5.0, 2.0, 0.0),
      );
      root.addOrReplaceChild(
        "left_sleeve",
        CubeListBuilder.create().texOffs(48, 48).addBox(-1.0, -2.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
        PartPose.offset(5.0, 2.0, 0.0),
      );
      root.addOrReplaceChild(
        "right_sleeve",
        CubeListBuilder.create().texOffs(40, 32).addBox(-3.0, -2.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
        PartPose.offset(-5.0, 2.0, 0.0),
      );
    }

    root.addOrReplaceChild(
      "left_leg",
      CubeListBuilder.create().texOffs(16, 48).addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation),
      PartPose.offset(1.9, 12.0, 0.0),
    );
    root.addOrReplaceChild(
      "left_pants",
      CubeListBuilder.create().texOffs(0, 48).addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
      PartPose.offset(1.9, 12.0, 0.0),
    );
    root.addOrReplaceChild(
      "right_pants",
      CubeListBuilder.create().texOffs(0, 32).addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
      PartPose.offset(-1.9, 12.0, 0.0),
    );
    root.addOrReplaceChild(
      "jacket",
      CubeListBuilder.create().texOffs(16, 32).addBox(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0, cubeDeformation.extend(0.25)),
      PartPose.ZERO,
    );
    return meshDefinition;
  }

  protected override bodyParts(): Iterable<ModelPart> {
    return [
      ...super.bodyParts(),
      this.leftPants,
      this.rightPants,
      this.leftSleeve,
      this.rightSleeve,
      this.jacket,
    ];
  }

  public renderEars(poseStack: PoseStack, buffer: VertexConsumer, packedLight: number, packedOverlay: number): void {
    this.ear.copyFrom(this.head);
    this.ear.x = 0.0;
    this.ear.y = 0.0;
    this.ear.render(poseStack, buffer, packedLight, packedOverlay);
  }

  public renderCloak(poseStack: PoseStack, buffer: VertexConsumer, packedLight: number, packedOverlay: number): void {
    this.cloak.render(poseStack, buffer, packedLight, packedOverlay);
  }

  public override setupAnim(entity: T, limbSwing: number, limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    this.crouching = entity.isCrouching();
    super.setupAnim(entity, limbSwing, limbSwingAmount, ageInTicks, netHeadYaw, headPitch);
    this.leftPants.copyFrom(this.leftLeg);
    this.rightPants.copyFrom(this.rightLeg);
    this.leftSleeve.copyFrom(this.leftArm);
    this.rightSleeve.copyFrom(this.rightArm);
    this.jacket.copyFrom(this.body);
    if (!entity.hasChestEquipment()) {
      if (entity.isCrouching()) {
        this.cloak.z = 1.4;
        this.cloak.y = 1.85;
      } else {
        this.cloak.z = 0.0;
        this.cloak.y = 0.0;
      }
    } else if (entity.isCrouching()) {
      this.cloak.z = 0.3;
      this.cloak.y = 0.8;
    } else {
      this.cloak.z = -1.1;
      this.cloak.y = -0.85;
    }
  }

  public override setAllVisible(visible: boolean): void {
    super.setAllVisible(visible);
    this.leftSleeve.visible = visible;
    this.rightSleeve.visible = visible;
    this.leftPants.visible = visible;
    this.rightPants.visible = visible;
    this.jacket.visible = visible;
    this.cloak.visible = visible;
    this.ear.visible = visible;
  }

  public override translateToHand(side: HumanoidArm, poseStack: PoseStack): void {
    const arm = this.getArm(side);
    if (this.slim) {
      const offset = 0.5 * (side === "right" ? 1 : -1);
      arm.x += offset;
      arm.translateAndRotate(poseStack);
      arm.x -= offset;
      return;
    }

    arm.translateAndRotate(poseStack);
  }

  public getRandomModelPart(random: { nextInt(bound: number): number }): ModelPart {
    return this.partsValue[random.nextInt(this.partsValue.length)]!;
  }
}
