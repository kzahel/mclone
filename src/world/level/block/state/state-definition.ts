import { Property } from "./properties/property";
import { StateHolder } from "./state-holder";

function stateKey(values: ReadonlyMap<Property<unknown>, unknown>): string {
  return [...values.entries()].map(([property, value]) => `${property.getName()}=${property.getNameForValue(value)}`).join(",");
}

export class StateDefinition<O, S extends StateHolder<O, S>> {
  public static readonly NAME_PATTERN = /^[a-z0-9_]+$/;

  private readonly propertiesByName: ReadonlyMap<string, Property<unknown>>;
  private readonly states: readonly S[];

  public constructor(
    private readonly defaultStateFactory: (owner: O) => S,
    private readonly owner: O,
    private readonly factory: StateDefinition.Factory<O, S>,
    properties: ReadonlyMap<string, Property<unknown>>,
  ) {
    this.propertiesByName = new Map([...properties.entries()].sort(([left], [right]) => left.localeCompare(right)));
    const propertyEntries = [...this.propertiesByName.values()];
    const statesByKey = new Map<string, S>();
    const states: S[] = [];

    const populate = (index: number, current: Map<Property<unknown>, unknown>): void => {
      if (index >= propertyEntries.length) {
        const values = new Map(current);
        const state = this.factory.create(this.owner, values, this.defaultStateFactory);
        statesByKey.set(stateKey(values), state);
        states.push(state);
        return;
      }

      const property = propertyEntries[index]!;
      for (const value of property.getPossibleValues()) {
        current.set(property, value);
        populate(index + 1, current);
      }
    };

    populate(0, new Map());
    for (const state of states) {
      state.populateNeighbours(statesByKey);
    }

    this.states = states;
  }

  public getPossibleStates(): readonly S[] {
    return this.states;
  }

  public any(): S {
    return this.states[0]!;
  }

  public getOwner(): O {
    return this.owner;
  }

  public getProperties(): readonly Property<unknown>[] {
    return [...this.propertiesByName.values()];
  }

  public getProperty(name: string): Property<unknown> | undefined {
    return this.propertiesByName.get(name);
  }

  public toString(): string {
    return `StateDefinition{owner=${this.owner}, properties=${this.getProperties().map((property) => property.getName()).join(",")}}`;
  }
}

export namespace StateDefinition {
  export class Builder<O, S extends StateHolder<O, S>> {
    private readonly properties = new Map<string, Property<unknown>>();

    public constructor(private readonly owner: O) {}

    public add(...properties: readonly Property<unknown>[]): Builder<O, S> {
      for (const property of properties) {
        this.validateProperty(property);
        this.properties.set(property.getName(), property);
      }

      return this;
    }

    private validateProperty<T>(property: Property<T>): void {
      const name = property.getName();
      if (!StateDefinition.NAME_PATTERN.test(name)) {
        throw new Error(`${this.owner} has invalidly named property: ${name}`);
      }

      const values = property.getPossibleValues();
      if (values.length <= 1) {
        throw new Error(`${this.owner} attempted use property ${name} with <= 1 possible values`);
      }

      for (const value of values) {
        const valueName = property.getNameForValue(value);
        if (!StateDefinition.NAME_PATTERN.test(valueName)) {
          throw new Error(`${this.owner} has property: ${name} with invalidly named value: ${valueName}`);
        }
      }

      if (this.properties.has(name)) {
        throw new Error(`${this.owner} has duplicate property: ${name}`);
      }
    }

    public create(defaultStateFactory: (owner: O) => S, factory: Factory<O, S>): StateDefinition<O, S> {
      return new StateDefinition(defaultStateFactory, this.owner, factory, this.properties);
    }
  }

  export interface Factory<O, S extends StateHolder<O, S>> {
    create(owner: O, values: ReadonlyMap<Property<unknown>, unknown>, defaultStateFactory: (owner: O) => S): S;
  }
}
