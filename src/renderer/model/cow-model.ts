import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { MeshDefinition } from "./geom/builders/mesh-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { QuadrupedModel } from "./quadruped-model";

export class CowModel<T = unknown> extends QuadrupedModel<T> {
  public constructor(root: ModelPart) {
    super(root, false, 10.0, 4.0, 2.0, 2.0, 24);
  }

  public static createBodyLayer(): LayerDefinition {
    const meshDefinition = new MeshDefinition();
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create()
        .texOffs(0, 0)
        .addBox(-4.0, -4.0, -6.0, 8.0, 8.0, 6.0)
        .texOffs(22, 0)
        .addBox("right_horn", -5.0, -5.0, -4.0, 1.0, 3.0, 1.0)
        .texOffs(22, 0)
        .addBox("left_horn", 4.0, -5.0, -4.0, 1.0, 3.0, 1.0),
      PartPose.offset(0.0, 4.0, -8.0),
    );
    root.addOrReplaceChild(
      "body",
      CubeListBuilder.create()
        .texOffs(18, 4)
        .addBox(-6.0, -10.0, -7.0, 12.0, 18.0, 10.0)
        .texOffs(52, 0)
        .addBox(-2.0, 2.0, -8.0, 4.0, 6.0, 1.0),
      PartPose.offsetAndRotation(0.0, 5.0, 2.0, Math.PI / 2, 0.0, 0.0),
    );
    const leg = CubeListBuilder.create().texOffs(0, 16).addBox(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0);
    root.addOrReplaceChild("right_hind_leg", leg, PartPose.offset(-4.0, 12.0, 7.0));
    root.addOrReplaceChild("left_hind_leg", leg, PartPose.offset(4.0, 12.0, 7.0));
    root.addOrReplaceChild("right_front_leg", leg, PartPose.offset(-4.0, 12.0, -6.0));
    root.addOrReplaceChild("left_front_leg", leg, PartPose.offset(4.0, 12.0, -6.0));
    return LayerDefinition.create(meshDefinition, 64, 32);
  }

  public getHead(): ModelPart {
    return this.head;
  }
}
