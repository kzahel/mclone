import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { LayerDefinition } from "./geom/builders/layer-definition";
import { ModelPart } from "./geom/model-part";
import { PartPose } from "./geom/part-pose";
import { QuadrupedModel } from "./quadruped-model";

export class PigModel<T = unknown> extends QuadrupedModel<T> {
  public constructor(root: ModelPart) {
    super(root, false, 4.0, 4.0, 2.0, 2.0, 24);
  }

  public static createBodyLayer(cubeDeformation: CubeDeformation): LayerDefinition {
    const meshDefinition = QuadrupedModel.createBodyMesh(6, cubeDeformation);
    const root = meshDefinition.getRoot();
    root.addOrReplaceChild(
      "head",
      CubeListBuilder.create()
        .texOffs(0, 0)
        .addBox(-4.0, -4.0, -8.0, 8.0, 8.0, 8.0, cubeDeformation)
        .texOffs(16, 16)
        .addBox(-2.0, 0.0, -9.0, 4.0, 3.0, 1.0, cubeDeformation),
      PartPose.offset(0.0, 12.0, -6.0),
    );
    return LayerDefinition.create(meshDefinition, 64, 32);
  }
}
