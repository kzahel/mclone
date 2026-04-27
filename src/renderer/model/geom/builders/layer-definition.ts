import { ModelPart } from "../model-part";
import { MaterialDefinition } from "./material-definition";
import { MeshDefinition } from "./mesh-definition";

export class LayerDefinition {
  private constructor(
    private readonly mesh: MeshDefinition,
    private readonly material: MaterialDefinition,
  ) {}

  public bakeRoot(): ModelPart {
    return this.mesh.getRoot().bake(this.material.xTexSize, this.material.yTexSize);
  }

  public static create(mesh: MeshDefinition, texWidth: number, texHeight: number): LayerDefinition {
    return new LayerDefinition(mesh, new MaterialDefinition(texWidth, texHeight));
  }
}
