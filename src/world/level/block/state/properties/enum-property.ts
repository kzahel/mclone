import type { StringRepresentable, StringRepresentableClass } from "../../../../../core/string-representable";
import { Property } from "./property";

type EnumPropertyFilter<T> = ((value: T) => boolean) | ReadonlyArray<T>;

function resolveValues<T extends StringRepresentable>(
  clazz: StringRepresentableClass<T>,
  valuesOrPredicate: readonly (T | EnumPropertyFilter<T>)[],
): readonly T[] {
  if (valuesOrPredicate.length === 0) {
    return [...clazz.values()];
  }

  if (valuesOrPredicate.length === 1) {
    const first = valuesOrPredicate[0]!;
    if (typeof first === "function") {
      return clazz.values().filter(first as (value: T) => boolean);
    }

    if (Array.isArray(first)) {
      return [...first];
    }
  }

  return valuesOrPredicate as readonly T[];
}

export class EnumProperty<T extends StringRepresentable> extends Property<T> {
  private readonly values: readonly T[];
  private readonly names = new Map<string, T>();

  protected constructor(name: string, clazz: StringRepresentableClass<T>, values: readonly T[]) {
    super(name, clazz);
    this.values = values;

    for (const value of values) {
      const serializedName = value.getSerializedName();
      if (this.names.has(serializedName)) {
        throw new Error(`Multiple values have the same name '${serializedName}'`);
      }

      this.names.set(serializedName, value);
    }
  }

  public static create<T extends StringRepresentable>(name: string, clazz: StringRepresentableClass<T>): EnumProperty<T>;
  public static create<T extends StringRepresentable>(
    name: string,
    clazz: StringRepresentableClass<T>,
    predicate: (value: T) => boolean,
  ): EnumProperty<T>;
  public static create<T extends StringRepresentable>(
    name: string,
    clazz: StringRepresentableClass<T>,
    values: ReadonlyArray<T>,
  ): EnumProperty<T>;
  public static create<T extends StringRepresentable>(name: string, clazz: StringRepresentableClass<T>, ...values: readonly T[]): EnumProperty<T>;
  public static create<T extends StringRepresentable>(
    name: string,
    clazz: StringRepresentableClass<T>,
    ...valuesOrPredicate: readonly (T | EnumPropertyFilter<T>)[]
  ): EnumProperty<T> {
    return new EnumProperty(name, clazz, resolveValues(clazz, valuesOrPredicate));
  }

  public override getPossibleValues(): readonly T[] {
    return this.values;
  }

  public override getValue(name: string): T | undefined {
    return this.names.get(name);
  }

  public override getNameForValue(value: T): string {
    return value.getSerializedName();
  }

  public override equals(other: unknown): boolean {
    return other instanceof EnumProperty && super.equals(other) && this.values.length === other.values.length;
  }

  public override generateHashCode(): number {
    return (31 * super.generateHashCode()) + this.values.length;
  }
}
