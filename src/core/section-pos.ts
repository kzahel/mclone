import { intFloorDiv } from "../util/mth";

export class SectionPos {
  public static posToSectionCoord(value: number): number {
    return SectionPos.blockToSectionCoord(Math.floor(value));
  }

  public static blockToSectionCoord(value: number): number {
    return intFloorDiv(value, 16);
  }

  public static sectionToBlockCoord(value: number): number {
    return value << 4;
  }

  public static sectionToBlockCoordWithOffset(value: number, offset: number): number {
    return SectionPos.sectionToBlockCoord(value) + offset;
  }
}
