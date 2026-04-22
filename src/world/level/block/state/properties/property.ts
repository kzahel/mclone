import type { StateHolder } from "../state-holder";

export type PropertyClass = object;

export abstract class Property<T> {
  private cachedHashCode: number | undefined;

  protected constructor(
    private readonly name: string,
    private readonly clazz: PropertyClass,
  ) {}

  public value(value: T): Property.Value<T>;
  public value(state: StateHolder<unknown, unknown>): Property.Value<T>;
  public value(valueOrState: T | StateHolder<unknown, unknown>): Property.Value<T> {
    if (valueOrState && typeof valueOrState === "object" && "getValue" in valueOrState && typeof valueOrState.getValue === "function") {
      return new Property.Value(this, valueOrState.getValue(this));
    }

    return new Property.Value(this, valueOrState as T);
  }

  public getAllValues(): Property.Value<T>[] {
    return this.getPossibleValues().map((value) => this.value(value));
  }

  public getName(): string {
    return this.name;
  }

  public getValueClass(): PropertyClass {
    return this.clazz;
  }

  public abstract getPossibleValues(): readonly T[];

  public abstract getNameForValue(value: T): string;

  public abstract getValue(name: string): T | undefined;

  public equals(other: unknown): boolean {
    return other instanceof Property && this.clazz === other.clazz && this.name === other.name;
  }

  public hashCode(): number {
    if (this.cachedHashCode === undefined) {
      this.cachedHashCode = this.generateHashCode();
    }

    return this.cachedHashCode;
  }

  public generateHashCode(): number {
    return (31 * Property.hashToken(this.clazz)) + Property.hashString(this.name);
  }

  public toString(): string {
    return `Property{name=${this.name}, values=${this.getPossibleValues().map((value) => this.getNameForValue(value)).join(",")}}`;
  }

  private static hashString(value: string): number {
    let hash = 0;
    for (let index = 0; index < value.length; index++) {
      hash = ((hash * 31) + value.charCodeAt(index)) | 0;
    }

    return hash;
  }

  public static hashToken(token: unknown): number {
    return Property.hashString(String(token));
  }
}

export namespace Property {
  export class Value<T> {
    public constructor(
      private readonly property: Property<T>,
      private readonly valueValue: T,
    ) {
      if (!property.getPossibleValues().includes(valueValue)) {
        throw new Error(`Value ${valueValue} does not belong to property ${property}`);
      }
    }

    public getProperty(): Property<T> {
      return this.property;
    }

    public value(): T {
      return this.valueValue;
    }

    public toString(): string {
      return `${this.property.getName()}=${this.property.getNameForValue(this.valueValue)}`;
    }

    public equals(other: unknown): boolean {
      return other instanceof Value && this.property === other.property && this.valueValue === other.valueValue;
    }

    public hashCode(): number {
      return (31 * this.property.hashCode()) + Property.hashToken(this.valueValue);
    }
  }
}
