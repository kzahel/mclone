import type { Block } from "../block";
import type { BlockState } from "./block-state";

export type BlockStateId = number;

export class BlockStateIdMap {
  private readonly idByState = new WeakMap<BlockState, BlockStateId>();

  public constructor(private readonly states: readonly BlockState[]) {
    for (let id = 0; id < states.length; id++) {
      const state = states[id]!;
      if (this.idByState.has(state)) {
        throw new Error(`Duplicate block state in id map: ${blockStateDebugKey(state)}`);
      }
      this.idByState.set(state, id);
    }
  }

  public get size(): number {
    return this.states.length;
  }

  public idFor(state: BlockState): BlockStateId {
    const id = this.idByState.get(state);
    if (id === undefined) {
      throw new Error(`Unknown block state: ${blockStateDebugKey(state)}`);
    }
    return id;
  }

  public stateFor(id: BlockStateId): BlockState {
    if (!Number.isInteger(id) || id < 0 || id >= this.states.length) {
      throw new Error(`BlockStateId ${id} out of bounds for ${this.states.length} states`);
    }
    return this.states[id]!;
  }

  public getStates(): readonly BlockState[] {
    return this.states;
  }
}

export function buildBlockStateIdMap(blocks: Iterable<Block>): BlockStateIdMap {
  const states: BlockState[] = [];
  for (const block of blocks) {
    states.push(...block.getStateDefinition().getPossibleStates());
  }
  return new BlockStateIdMap(states);
}

export function blockStateDebugKey(state: BlockState): string {
  const blockName = state.getBlock().getLocation()?.toString() ?? state.getBlock().toString();
  const properties = [...state.getValues().entries()]
    .map(([property, value]) => [property.getName(), property.getNameForValue(value)] as const)
    .sort(([left], [right]) => left.localeCompare(right));

  if (properties.length === 0) {
    return blockName;
  }

  return `${blockName}[${properties.map(([key, value]) => `${key}=${value}`).join(",")}]`;
}
