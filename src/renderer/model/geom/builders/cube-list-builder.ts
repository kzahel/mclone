import { CubeDefinition } from "./cube-definition";
import { CubeDeformation } from "./cube-deformation";

type AddBoxArgs =
  | [originX: number, originY: number, originZ: number, dimensionX: number, dimensionY: number, dimensionZ: number]
  | [comment: string, originX: number, originY: number, originZ: number, dimensionX: number, dimensionY: number, dimensionZ: number]
  | [
      comment: string,
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      cubeDeformation: CubeDeformation,
    ]
  | [
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      cubeDeformation: CubeDeformation,
    ]
  | [
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      mirror: boolean,
    ]
  | [
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      cubeDeformation: CubeDeformation,
      texScaleU: number,
      texScaleV: number,
    ]
  | [
      comment: string,
      originX: number,
      originY: number,
      originZ: number,
      dimensionX: number,
      dimensionY: number,
      dimensionZ: number,
      cubeDeformation: CubeDeformation,
      xTexOffs: number,
      yTexOffs: number,
    ];

export class CubeListBuilder {
  private readonly cubes: CubeDefinition[] = [];
  private xTexOffs = 0;
  private yTexOffs = 0;
  private mirrorValue = false;

  public texOffs(xTexOffs: number, yTexOffs: number): CubeListBuilder {
    this.xTexOffs = xTexOffs;
    this.yTexOffs = yTexOffs;
    return this;
  }

  public mirror(): CubeListBuilder;
  public mirror(mirror: boolean): CubeListBuilder;
  public mirror(mirror = true): CubeListBuilder {
    this.mirrorValue = mirror;
    return this;
  }

  public addBox(originX: number, originY: number, originZ: number, dimensionX: number, dimensionY: number, dimensionZ: number): CubeListBuilder;
  public addBox(
    comment: string,
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
  ): CubeListBuilder;
  public addBox(
    comment: string,
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    cubeDeformation: CubeDeformation,
  ): CubeListBuilder;
  public addBox(
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    cubeDeformation: CubeDeformation,
  ): CubeListBuilder;
  public addBox(
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    mirror: boolean,
  ): CubeListBuilder;
  public addBox(
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    cubeDeformation: CubeDeformation,
    texScaleU: number,
    texScaleV: number,
  ): CubeListBuilder;
  public addBox(
    comment: string,
    originX: number,
    originY: number,
    originZ: number,
    dimensionX: number,
    dimensionY: number,
    dimensionZ: number,
    cubeDeformation: CubeDeformation,
    xTexOffs: number,
    yTexOffs: number,
  ): CubeListBuilder;
  public addBox(...args: AddBoxArgs): CubeListBuilder {
    let comment: string | null = null;
    let offset = 0;
    if (typeof args[0] === "string") {
      comment = args[0];
      offset = 1;
    }

    const originX = args[offset] as number;
    const originY = args[offset + 1] as number;
    const originZ = args[offset + 2] as number;
    const dimensionX = args[offset + 3] as number;
    const dimensionY = args[offset + 4] as number;
    const dimensionZ = args[offset + 5] as number;
    const seventh = args[offset + 6] as CubeDeformation | boolean | undefined;
    let cubeDeformation = CubeDeformation.NONE;
    let mirror = this.mirrorValue;
    let texScaleU = 1.0;
    let texScaleV = 1.0;

    if (seventh instanceof CubeDeformation) {
      cubeDeformation = seventh;
      const eighth = args[offset + 7] as number | undefined;
      const ninth = args[offset + 8] as number | undefined;
      if (comment !== null && eighth !== undefined && ninth !== undefined) {
        this.texOffs(eighth, ninth);
      } else if (eighth !== undefined && ninth !== undefined) {
        texScaleU = eighth;
        texScaleV = ninth;
      }
    } else if (typeof seventh === "boolean") {
      mirror = seventh;
    }

    this.cubes.push(
      new CubeDefinition(
        comment,
        this.xTexOffs,
        this.yTexOffs,
        originX,
        originY,
        originZ,
        dimensionX,
        dimensionY,
        dimensionZ,
        cubeDeformation,
        mirror,
        texScaleU,
        texScaleV,
      ),
    );
    return this;
  }

  public getCubes(): readonly CubeDefinition[] {
    return [...this.cubes];
  }

  public static create(): CubeListBuilder {
    return new CubeListBuilder();
  }
}
