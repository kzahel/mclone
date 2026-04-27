import { PartPose } from "../part-pose";
import { PartDefinition } from "./part-definition";

export class MeshDefinition {
  private readonly root = new PartDefinition([], PartPose.ZERO);

  public getRoot(): PartDefinition {
    return this.root;
  }
}
