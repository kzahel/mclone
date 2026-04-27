import { HumanoidModel } from "./humanoid-model";
import { PartPose } from "./geom/part-pose";
import { CubeDeformation } from "./geom/builders/cube-deformation";
import { CubeListBuilder } from "./geom/builders/cube-list-builder";
import type { MeshDefinition } from "./geom/builders/mesh-definition";

export class PlayerModel {
  public static createMesh(cubeDeformation: CubeDeformation, slim: boolean): MeshDefinition {
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
}
