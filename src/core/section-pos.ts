import { intFloorDiv } from "../util/mth";

export class SectionPos {
  public static blockToSectionCoord(value: number): number {
    return intFloorDiv(value, 16);
  }

  public static sectionToBlockCoord(value: number): number {
    return value << 4;
  }
}
