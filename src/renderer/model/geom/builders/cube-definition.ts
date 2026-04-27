import { Vector3f } from "../../../math/vector3f";
import { ModelPart } from "../model-part";
import { CubeDeformation } from "./cube-deformation";
import { UVPair } from "./uv-pair";

export class CubeDefinition {
  public readonly comment: string | null;
  private readonly origin: Vector3f;
  private readonly dimensions: Vector3f;
  private readonly grow: CubeDeformation;
  private readonly mirrorValue: boolean;
  private readonly texCoord: UVPair;
  private readonly texScale: UVPair;

  public constructor(
    comment: string | null,
    texCoordU: number,
    texCoordV: number,
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    cubeDeformation: CubeDeformation,
    mirror: boolean,
    texScaleU: number,
    texScaleV: number,
  ) {
    this.comment = comment;
    this.texCoord = new UVPair(texCoordU, texCoordV);
    this.origin = new Vector3f(originX, originY, originZ);
    this.dimensions = new Vector3f(dimensionX, dimensionY, dimensionZ);
    this.grow = cubeDeformation;
    this.mirrorValue = mirror;
    this.texScale = new UVPair(texScaleU, texScaleV);
  }

  public bake(texWidth: number, texHeight: number): ModelPart.Cube {
    return new ModelPart.Cube(
      Math.trunc(this.texCoord.u()),
      Math.trunc(this.texCoord.v()),
      this.origin.x(),
      this.origin.y(),
      this.origin.z(),
      this.dimensions.x(),
      this.dimensions.y(),
      this.dimensions.z(),
      this.grow.growX,
      this.grow.growY,
      this.grow.growZ,
      this.mirrorValue,
      texWidth * this.texScale.u(),
      texHeight * this.texScale.v(),
    );
  }
}
