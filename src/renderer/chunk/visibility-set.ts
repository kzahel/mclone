import { Direction } from "../../core/direction";

const FACINGS = Direction.values().length;

export class VisibilitySet {
  private readonly data = new Array<boolean>(FACINGS * FACINGS).fill(false);

  public add(directions: ReadonlySet<Direction>): void {
    for (const first of directions) {
      for (const second of directions) {
        this.set(first, second, true);
      }
    }
  }

  public set(first: Direction, second: Direction, value: boolean): void {
    const firstIndex = first.get3DDataValue();
    const secondIndex = second.get3DDataValue();
    this.data[firstIndex + (secondIndex * FACINGS)] = value;
    this.data[secondIndex + (firstIndex * FACINGS)] = value;
  }

  public setAll(value: boolean): void {
    this.data.fill(value);
  }

  public visibilityBetween(first: Direction, second: Direction): boolean {
    return this.data[first.get3DDataValue() + (second.get3DDataValue() * FACINGS)] ?? false;
  }

  public toString(): string {
    let result = " ";
    for (const direction of Direction.values()) {
      result += ` ${direction.toString().toUpperCase().charAt(0)}`;
    }

    result += "\n";
    for (const row of Direction.values()) {
      result += row.toString().toUpperCase().charAt(0);
      for (const column of Direction.values()) {
        if (row === column) {
          result += "  ";
        } else {
          result += ` ${this.visibilityBetween(row, column) ? "Y" : "n"}`;
        }
      }

      result += "\n";
    }

    return result;
  }
}
