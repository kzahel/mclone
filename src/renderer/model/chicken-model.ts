import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { AgeableListModel } from "./ageable-list-model";

export class ChickenModel<T = unknown> extends AgeableListModel<T> {
  public static readonly RED_THING = "red_thing";
  private readonly head: ModelPart;
  private readonly body: ModelPart;
  private readonly rightLeg: ModelPart;
  private readonly leftLeg: ModelPart;
  private readonly rightWing: ModelPart;
  private readonly leftWing: ModelPart;
  private readonly beak: ModelPart;
  private readonly redThing: ModelPart;

  public constructor(root: ModelPart) {
    super();
    this.head = root.getChild("head");
    this.beak = root.getChild("beak");
    this.redThing = root.getChild(ChickenModel.RED_THING);
    this.body = root.getChild("body");
    this.rightLeg = root.getChild("right_leg");
    this.leftLeg = root.getChild("left_leg");
    this.rightWing = root.getChild("right_wing");
    this.leftWing = root.getChild("left_wing");
  }

  public static createBodyLayer(): LayerDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create().texOffs(0, 0).addBox(-2.0, -6.0, -2.0, 4.0, 6.0, 3.0),
      PartPose.offset(0.0, 15.0, -4.0),
    );
    root.addOrReplaceChild(
      "beak",
      CubeListBuilder.create().texOffs(14, 0).addBox(-2.0, -4.0, -4.0, 4.0, 2.0, 2.0),
      PartPose.offset(0.0, 15.0, -4.0),
    );
    root.addOrReplaceChild(
      ChickenModel.RED_THING,
      CubeListBuilder.create().texOffs(14, 4).addBox(-1.0, -2.0, -3.0, 2.0, 2.0, 2.0),
      PartPose.offset(0.0, 15.0, -4.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create().texOffs(0, 9).addBox(-3.0, -4.0, -3.0, 6.0, 8.0, 6.0),
      PartPose.offsetAndRotation(0.0, 16.0, 0.0, Math.PI / 2, 0.0, 0.0),
    );
    const leg = CubeListBuilder.create().texOffs(26, 0).addBox(-1.0, 0.0, -3.0, 3.0, 5.0, 3.0);
    root.addOrReplaceChild("right_leg", leg, PartPose.offset(-2.0, 19.0, 1.0));
    root.addOrReplaceChild("left_leg", leg, PartPose.offset(1.0, 19.0, 1.0));
    root.addOrReplaceChild(
      "right_wing",
      CubeListBuilder.create().texOffs(24, 13).addBox(0.0, 0.0, -3.0, 1.0, 4.0, 6.0),
      PartPose.offset(-4.0, 13.0, 0.0),
    );
    root.addOrReplaceChild(
      "left_wing",
      CubeListBuilder.create().texOffs(24, 13).addBox(-1.0, 0.0, -3.0, 1.0, 4.0, 6.0),
      PartPose.offset(4.0, 13.0, 0.0),
    );
    return LayerDefinition.create(meshDefinition, 64, 32);
  }

  protected override headParts(): Iterable<ModelPart> {
    return [this.head, this.beak, this.redThing];
  }

  protected override bodyParts(): Iterable<ModelPart> {
    return [this.body, this.rightLeg, this.leftLeg, this.rightWing, this.leftWing];
  }

  public override setupAnim(_entity: T, limbSwing: number, limbSwingAmount: number, ageInTicks: number, netHeadYaw: number, headPitch: number): void {
    this.head.xRot = headPitch * (Math.PI / 180.0);
    this.head.yRot = netHeadYaw * (Math.PI / 180.0);
    this.beak.xRot = this.head.xRot;
    this.beak.yRot = this.head.yRot;
    this.redThing.xRot = this.head.xRot;
    this.redThing.yRot = this.head.yRot;
    this.rightLeg.xRot = Math.cos(limbSwing * 0.6662) * 1.4 * limbSwingAmount;
    this.leftLeg.xRot = Math.cos((limbSwing * 0.6662) + Math.PI) * 1.4 * limbSwingAmount;
    this.rightWing.zRot = ageInTicks;
    this.leftWing.zRot = -ageInTicks;
  }
}
