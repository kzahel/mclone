import { Property } from "./property";

export class BooleanProperty extends Property<boolean> {
  private readonly values = [true, false] as const;

  protected constructor(name: string) {
    super(name, BooleanProperty);
  }

  public static create(name: string): BooleanProperty {
    return new BooleanProperty(name);
  }

  public override getPossibleValues(): readonly boolean[] {
    return this.values;
  }

  public override getValue(name: string): boolean | undefined {
    if (name === "true") {
      return true;
    }

    if (name === "false") {
      return false;
    }

    return undefined;
  }

  public override getNameForValue(value: boolean): string {
    return value.toString();
  }

  public override equals(other: unknown): boolean {
    return other instanceof BooleanProperty && super.equals(other);
  }

  public override generateHashCode(): number {
    return (31 * super.generateHashCode()) + 1231;
  }
}
