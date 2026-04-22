import { ResourceLocation } from "../../core/resource-location";

function decompose(value: string): [string | null, string, string] {
  const result: [string | null, string, string] = [null, value, ""];
  const variantSeparator = value.indexOf("#");
  let location = value;
  if (variantSeparator >= 0) {
    result[2] = value.substring(variantSeparator + 1);
    if (variantSeparator > 1) {
      location = value.substring(0, variantSeparator);
    }
  }

  const namespaceSeparator = location.indexOf(":");
  if (namespaceSeparator >= 0) {
    result[1] = location.substring(namespaceSeparator + 1);
    if (namespaceSeparator >= 1) {
      result[0] = location.substring(0, namespaceSeparator);
    }
  }

  return result;
}

export class ModelResourceLocation extends ResourceLocation {
  private readonly variant: string;

  public constructor(parts: [string | null, string, string]);
  public constructor(namespace: string, path: string, variant: string);
  public constructor(location: string);
  public constructor(location: ResourceLocation, variant: string);
  public constructor(first: [string | null, string, string] | string | ResourceLocation, second?: string, third?: string) {
    if (Array.isArray(first)) {
      super(first[0] ?? ResourceLocation.DEFAULT_NAMESPACE, first[1]);
      this.variant = first[2].toLowerCase();
      return;
    }

    if (first instanceof ResourceLocation) {
      const value = decompose(`${first.toString()}#${second ?? ""}`);
      super(value[0] ?? ResourceLocation.DEFAULT_NAMESPACE, value[1]);
      this.variant = value[2].toLowerCase();
      return;
    }

    if (third !== undefined) {
      super(first, second!);
      this.variant = third.toLowerCase();
      return;
    }

    const value = decompose(first);
    super(value[0] ?? ResourceLocation.DEFAULT_NAMESPACE, value[1]);
    this.variant = value[2].toLowerCase();
  }

  public getVariant(): string {
    return this.variant;
  }

  public override equals(other: unknown): boolean {
    return other instanceof ModelResourceLocation && super.equals(other) && this.variant === other.variant;
  }

  public override toString(): string {
    return `${super.toString()}#${this.variant}`;
  }
}
