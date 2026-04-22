import type { Block } from "../../../world/level/block/block";
import type { BlockState } from "../../../world/level/block/state/block-state";
import type { StateDefinition } from "../../../world/level/block/state/state-definition";
import { type Condition } from "./condition";

export class KeyValueCondition implements Condition {
  public constructor(
    private readonly key: string,
    private readonly value: string,
  ) {}

  public getPredicate(definition: StateDefinition<Block, BlockState>): (state: BlockState) => boolean {
    const property = definition.getProperty(this.key);
    if (property === undefined) {
      throw new Error(`Unknown property '${this.key}' on '${definition.getOwner()}'`);
    }

    let value = this.value;
    const negated = value.length > 0 && value.charAt(0) === "!";
    if (negated) {
      value = value.substring(1);
    }

    const parts = value.split("|").filter((part) => part.length > 0);
    if (parts.length === 0) {
      throw new Error(`Empty value '${this.value}' for property '${this.key}' on '${definition.getOwner()}'`);
    }

    const predicates = parts.map((part) => {
      const parsedValue = property.getValue(part);
      if (parsedValue === undefined) {
        throw new Error(`Unknown value '${part}' for property '${this.key}' on '${definition.getOwner()}' in '${this.value}'`);
      }

      return (state: BlockState) => state.getValue(property) === parsedValue;
    });

    const predicate = predicates.length === 1
      ? predicates[0]!
      : (state: BlockState) => predicates.some((entry) => entry(state));

    return negated ? (state) => !predicate(state) : predicate;
  }

  public toString(): string {
    return `KeyValueCondition{key=${this.key}, value=${this.value}}`;
  }
}
