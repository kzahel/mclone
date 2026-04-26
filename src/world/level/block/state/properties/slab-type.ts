import type { StringRepresentable, StringRepresentableClass } from "../../../../../core/string-representable";

class SlabTypeValue implements StringRepresentable {
  public constructor(private readonly name: string) {}

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

const TOP = new SlabTypeValue("top");
const BOTTOM = new SlabTypeValue("bottom");
const DOUBLE = new SlabTypeValue("double");
const VALUES = [TOP, BOTTOM, DOUBLE] as const;

export const SlabType = {
  TOP,
  BOTTOM,
  DOUBLE,
  values(): readonly SlabTypeValue[] {
    return VALUES;
  },
} satisfies StringRepresentableClass<SlabTypeValue> & {
  TOP: SlabTypeValue;
  BOTTOM: SlabTypeValue;
  DOUBLE: SlabTypeValue;
};

export type SlabType = (typeof VALUES)[number];
