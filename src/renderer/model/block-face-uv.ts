import { convertToNumber, expectJsonObject, getAsJsonArray, getAsNumber, hasJsonValue, type JsonObject } from "./model-json-utils";

export class BlockFaceUV {
  public constructor(
    public uvs: number[] | undefined,
    public readonly rotation: number,
  ) {}

  public getU(index: number): number {
    if (this.uvs === undefined) {
      throw new Error("uvs");
    }

    const shiftedIndex = this.getShiftedIndex(index);
    return this.uvs[shiftedIndex !== 0 && shiftedIndex !== 1 ? 2 : 0]!;
  }

  public getV(index: number): number {
    if (this.uvs === undefined) {
      throw new Error("uvs");
    }

    const shiftedIndex = this.getShiftedIndex(index);
    return this.uvs[shiftedIndex !== 0 && shiftedIndex !== 3 ? 3 : 1]!;
  }

  private getShiftedIndex(index: number): number {
    return (index + Math.trunc(this.rotation / 90)) % 4;
  }

  public getReverseIndex(index: number): number {
    return (index + 4 - Math.trunc(this.rotation / 90)) % 4;
  }

  public setMissingUv(uvs: number[]): void {
    if (this.uvs === undefined) {
      this.uvs = uvs;
    }
  }

  public static fromJson(value: unknown): BlockFaceUV {
    const json = expectJsonObject(value, "face");
    const uvs = BlockFaceUV.getUvs(json);
    const rotation = BlockFaceUV.getRotation(json);
    return new BlockFaceUV(uvs, rotation);
  }

  private static getRotation(json: JsonObject): number {
    const rotation = getAsNumber(json, "rotation", 0);
    if (rotation >= 0 && rotation % 90 === 0 && Math.trunc(rotation / 90) <= 3) {
      return rotation;
    }

    throw new Error(`Invalid rotation ${rotation} found, only 0/90/180/270 allowed`);
  }

  private static getUvs(json: JsonObject): number[] | undefined {
    if (!hasJsonValue(json, "uv")) {
      return undefined;
    }

    const uvArray = getAsJsonArray(json, "uv");
    if (uvArray.length !== 4) {
      throw new Error(`Expected 4 uv values, found: ${uvArray.length}`);
    }

    return uvArray.map((entry, index) => convertToNumber(entry, `uv[${index}]`));
  }
}
