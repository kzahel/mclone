import { Direction } from "../../../core/direction";

export class BlockPlaceContext {
  public constructor(private readonly clickedFace: Direction) {}

  public getClickedFace(): Direction {
    return this.clickedFace;
  }
}
