import type { StringRepresentable, StringRepresentableClass } from "../../../../../core/string-representable";

class DripstoneThicknessValue implements StringRepresentable {
  public constructor(private readonly name: string) {}

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

const TIP_MERGE = new DripstoneThicknessValue("tip_merge");
const TIP = new DripstoneThicknessValue("tip");
const FRUSTUM = new DripstoneThicknessValue("frustum");
const MIDDLE = new DripstoneThicknessValue("middle");
const BASE = new DripstoneThicknessValue("base");
const VALUES = [TIP_MERGE, TIP, FRUSTUM, MIDDLE, BASE] as const;

export const DripstoneThickness = {
  TIP_MERGE,
  TIP,
  FRUSTUM,
  MIDDLE,
  BASE,
  values(): readonly DripstoneThicknessValue[] {
    return VALUES;
  },
} satisfies StringRepresentableClass<DripstoneThicknessValue> & {
  TIP_MERGE: DripstoneThicknessValue;
  TIP: DripstoneThicknessValue;
  FRUSTUM: DripstoneThicknessValue;
  MIDDLE: DripstoneThicknessValue;
  BASE: DripstoneThicknessValue;
};

export type DripstoneThickness = (typeof VALUES)[number];
