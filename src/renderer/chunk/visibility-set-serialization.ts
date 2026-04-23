import { Direction } from "../../core/direction";
import { VisibilitySet } from "./visibility-set";

const DIRECTIONS = Direction.values();
const SERIALIZED_VISIBILITY_SET_SIZE = DIRECTIONS.length * DIRECTIONS.length;

export type SerializedVisibilitySet = readonly boolean[];

export function serializeVisibilitySet(visibilitySet: VisibilitySet): SerializedVisibilitySet {
  const serialized: boolean[] = [];
  for (const second of DIRECTIONS) {
    for (const first of DIRECTIONS) {
      serialized.push(visibilitySet.visibilityBetween(first, second));
    }
  }

  return serialized;
}

export function deserializeVisibilitySet(serialized: SerializedVisibilitySet): VisibilitySet {
  if (serialized.length !== SERIALIZED_VISIBILITY_SET_SIZE) {
    throw new Error(
      `Serialized visibility set had ${serialized.length} cells instead of ${SERIALIZED_VISIBILITY_SET_SIZE}`,
    );
  }

  const visibilitySet = new VisibilitySet();
  let index = 0;
  for (const second of DIRECTIONS) {
    for (const first of DIRECTIONS) {
      if (serialized[index++]) {
        visibilitySet.set(first, second, true);
      }
    }
  }

  return visibilitySet;
}
