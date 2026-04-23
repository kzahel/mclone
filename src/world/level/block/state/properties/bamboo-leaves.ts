import type { StringRepresentable, StringRepresentableClass } from "../../../../../core/string-representable";

class BambooLeavesValue implements StringRepresentable {
  public constructor(private readonly name: string) {}

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

const NONE = new BambooLeavesValue("none");
const SMALL = new BambooLeavesValue("small");
const LARGE = new BambooLeavesValue("large");
const VALUES = [NONE, SMALL, LARGE] as const;

export const BambooLeaves = {
  NONE,
  SMALL,
  LARGE,
  values(): readonly BambooLeavesValue[] {
    return VALUES;
  },
} satisfies StringRepresentableClass<BambooLeavesValue> & {
  NONE: BambooLeavesValue;
  SMALL: BambooLeavesValue;
  LARGE: BambooLeavesValue;
};

export type BambooLeaves = (typeof VALUES)[number];
