import { Property } from "./properties/property";

function findNextInCollection<T>(values: readonly T[], current: T): T {
  const index = values.findIndex((value) => value === current);
  if (index === -1) {
    throw new Error(`Unable to find ${current} in property values`);
  }

  return values[(index + 1) % values.length]!;
}

function propertyEntryToString<T>(property: Property<T>, value: T): string {
  return `${property.getName()}=${property.getNameForValue(value)}`;
}

export abstract class StateHolder<O, S> {
  protected static readonly NAME_TAG = "Name";
  protected static readonly PROPERTIES_TAG = "Properties";

  private neighbours: Map<Property<unknown>, Map<unknown, S>> | undefined;

  protected constructor(
    protected readonly owner: O,
    private readonly values: ReadonlyMap<Property<unknown>, unknown>,
  ) {}

  public cycle<T>(property: Property<T>): S {
    return this.setValue(property, findNextInCollection(property.getPossibleValues(), this.getValue(property)));
  }

  public getProperties(): readonly Property<unknown>[] {
    return [...this.values.keys()];
  }

  public hasProperty<T>(property: Property<T>): boolean {
    return this.values.has(property);
  }

  public getValue<T>(property: Property<T>): T {
    const value = this.values.get(property);
    if (value === undefined) {
      throw new Error(`Cannot get property ${property} as it does not exist in ${this.owner}`);
    }

    return value as T;
  }

  public getOptionalValue<T>(property: Property<T>): T | undefined {
    return this.values.get(property) as T | undefined;
  }

  public setValue<T>(property: Property<T>, value: T): S {
    const current = this.values.get(property);
    if (current === undefined) {
      throw new Error(`Cannot set property ${property} as it does not exist in ${this.owner}`);
    }

    if (current === value) {
      return this as unknown as S;
    }

    const neighbour = this.neighbours?.get(property)?.get(value);
    if (neighbour === undefined) {
      throw new Error(`Cannot set property ${property} to ${value} on ${this.owner}, it is not an allowed value`);
    }

    return neighbour;
  }

  public populateNeighbours(states: ReadonlyMap<string, S>): void {
    if (this.neighbours !== undefined) {
      throw new Error("Neighbours already populated");
    }

    const neighbours = new Map<Property<unknown>, Map<unknown, S>>();
    for (const [property, currentValue] of this.values.entries()) {
      const propertyNeighbours = new Map<unknown, S>();
      for (const value of property.getPossibleValues()) {
        if (value === currentValue) {
          continue;
        }

        const state = states.get(this.makeNeighbourKey(property, value));
        if (state !== undefined) {
          propertyNeighbours.set(value, state);
        }
      }

      neighbours.set(property, propertyNeighbours);
    }

    this.neighbours = neighbours;
  }

  private makeNeighbourKey(property: Property<unknown>, value: unknown): string {
    const entries = [...this.values.entries()].map(([entryProperty, entryValue]) =>
      propertyEntryToString(entryProperty, entryProperty === property ? value : entryValue),
    );
    return entries.join(",");
  }

  public getValues(): ReadonlyMap<Property<unknown>, unknown> {
    return this.values;
  }

  public toString(): string {
    const entries = [...this.values.entries()].map(([property, value]) => propertyEntryToString(property, value));
    return entries.length === 0 ? `${this.owner}` : `${this.owner}[${entries.join(",")}]`;
  }
}
