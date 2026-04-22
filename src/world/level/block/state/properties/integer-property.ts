import { Property } from "./property";

export class IntegerProperty extends Property<number> {
  private readonly values: readonly number[];

  protected constructor(name: string, min: number, max: number) {
    super(name, IntegerProperty);
    if (min < 0) {
      throw new Error(`Min value of ${name} must be 0 or greater`);
    }

    if (max <= min) {
      throw new Error(`Max value of ${name} must be greater than min (${min})`);
    }

    const values: number[] = [];
    for (let value = min; value <= max; value++) {
      values.push(value);
    }

    this.values = values;
  }

  public static create(name: string, min: number, max: number): IntegerProperty {
    return new IntegerProperty(name, min, max);
  }

  public override getPossibleValues(): readonly number[] {
    return this.values;
  }

  public override getValue(name: string): number | undefined {
    const parsed = Number.parseInt(name, 10);
    if (Number.isNaN(parsed)) {
      return undefined;
    }

    return this.values.includes(parsed) ? parsed : undefined;
  }

  public override getNameForValue(value: number): string {
    return value.toString();
  }

  public override equals(other: unknown): boolean {
    return other instanceof IntegerProperty && super.equals(other) && this.values.length === other.values.length;
  }

  public override generateHashCode(): number {
    return (31 * super.generateHashCode()) + this.values.length;
  }
}
