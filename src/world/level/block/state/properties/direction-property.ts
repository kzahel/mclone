import { Direction } from "../../../../../core/direction";
import { EnumProperty } from "./enum-property";

type DirectionFilter = ((direction: Direction) => boolean) | { test(direction: Direction | undefined): boolean } | ReadonlyArray<Direction>;

function resolveDirectionFilter(filter: DirectionFilter): (direction: Direction) => boolean {
  if (typeof filter === "function") {
    return filter;
  }

  if (Array.isArray(filter)) {
    return (direction) => filter.includes(direction);
  }

  return (direction) => (filter as { test(direction: Direction | undefined): boolean }).test(direction);
}

export class DirectionProperty extends EnumProperty<Direction> {
  protected constructor(name: string, values: readonly Direction[]) {
    super(name, Direction, values);
  }

  public static override create(name: string): DirectionProperty;
  public static override create(name: string, predicate: DirectionFilter): DirectionProperty;
  public static override create(name: string, ...values: readonly Direction[]): DirectionProperty;
  public static override create(name: string, ...valuesOrPredicate: readonly (Direction | DirectionFilter)[]): DirectionProperty {
    if (valuesOrPredicate.length === 0) {
      return new DirectionProperty(name, [...Direction.values()]);
    }

    if (valuesOrPredicate.length === 1) {
      const first = valuesOrPredicate[0]!;
      if (typeof first === "function" || Array.isArray(first) || (typeof first === "object" && "test" in first)) {
        return new DirectionProperty(name, Direction.values().filter(resolveDirectionFilter(first as DirectionFilter)));
      }
    }

    return new DirectionProperty(name, valuesOrPredicate as readonly Direction[]);
  }
}
