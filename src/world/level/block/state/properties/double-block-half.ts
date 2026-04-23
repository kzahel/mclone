import type { StringRepresentable, StringRepresentableClass } from "../../../../../core/string-representable";

class DoubleBlockHalfValue implements StringRepresentable {
  public constructor(private readonly name: string) {}

  public getSerializedName(): string {
    return this.name;
  }

  public toString(): string {
    return this.name;
  }
}

const UPPER = new DoubleBlockHalfValue("upper");
const LOWER = new DoubleBlockHalfValue("lower");
const VALUES = [UPPER, LOWER] as const;

export const DoubleBlockHalf = {
  UPPER,
  LOWER,
  values(): readonly DoubleBlockHalfValue[] {
    return VALUES;
  },
} satisfies StringRepresentableClass<DoubleBlockHalfValue> & {
  UPPER: DoubleBlockHalfValue;
  LOWER: DoubleBlockHalfValue;
};

export type DoubleBlockHalf = (typeof VALUES)[number];
