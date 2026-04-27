import { PartPose } from "./geom/part-pose";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import { MeshDefinition } from "./geom/builders/mesh-definition";

export class HumanoidModel {
  public static createMesh(cubeDeformation: CubeDeformation, yOffset: number): MeshDefinition {
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
}
