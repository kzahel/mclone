import { ModelPart } from "../model-part";
import { PartPose } from "../part-pose";
import type { CubeDefinition } from "./cube-definition";
import { CubeListBuilder } from "./cube-list-builder";

export class PartDefinition {
  private readonly children = new Map<string, PartDefinition>();

  public constructor(
    private readonly cubes: readonly CubeDefinition[],
    private readonly partPose: PartPose,
  ) {}

  public addOrReplaceChild(name: string, cubes: CubeListBuilder, partPose: PartPose): PartDefinition {
    const child = new PartDefinition(cubes.getCubes(), partPose);
    const oldChild = this.children.get(name);
    this.children.set(name, child);
    if (oldChild !== undefined) {
      for (const [childName, grandChild] of oldChild.children) {
        child.children.set(childName, grandChild);
      }
    }

    return child;
  }

  public bake(texWidth: number, texHeight: number): ModelPart {
    const children = new Map<string, ModelPart>();
    for (const [name, child] of this.children) {
      children.set(name, child.bake(texWidth, texHeight));
    }

    const cubes = this.cubes.map((cube) => cube.bake(texWidth, texHeight));
    const part = new ModelPart(cubes, children);
    part.loadPose(this.partPose);
    return part;
  }

  public getChild(name: string): PartDefinition | undefined {
    return this.children.get(name);
  }
}
