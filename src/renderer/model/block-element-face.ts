import { Direction } from "../../core/direction";
import { BlockFaceUV } from "./block-face-uv";
import { expectJsonObject, getAsNumber, getAsString, type JsonObject } from "./model-json-utils";

export class BlockElementFace {
  public static readonly NO_TINT = -1;

  public constructor(
    public readonly cullForDirection: Direction | undefined,
    public readonly tintIndex: number,
    public readonly texture: string,
    public readonly uv: BlockFaceUV,
  ) {}

  public static fromJson(value: unknown): BlockElementFace {
    const json = expectJsonObject(value, "face");
    const cullForDirection = BlockElementFace.getCullFacing(json);
    const tintIndex = BlockElementFace.getTintIndex(json);
    const texture = BlockElementFace.getTexture(json);
    const uv = BlockFaceUV.fromJson(json);
    return new BlockElementFace(cullForDirection, tintIndex, texture, uv);
  }

  private static getTintIndex(json: JsonObject): number {
    return getAsNumber(json, "tintindex", -1);
  }

  private static getTexture(json: JsonObject): string {
    return getAsString(json, "texture");
  }

  private static getCullFacing(json: JsonObject): Direction | undefined {
    const cullFace = getAsString(json, "cullface", "");
    return Direction.byName(cullFace);
  }
}
