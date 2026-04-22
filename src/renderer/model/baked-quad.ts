import { Direction } from "../../core/direction";
import { TextureAtlasSprite } from "../texture/texture-atlas-sprite";

export class BakedQuad {
  public constructor(
    protected readonly vertices: number[],
    protected readonly tintIndex: number,
    protected readonly direction: Direction,
    protected readonly sprite: TextureAtlasSprite,
    private readonly shade: boolean,
  ) {}

  public getSprite(): TextureAtlasSprite {
    return this.sprite;
  }

  public getVertices(): readonly number[] {
    return this.vertices;
  }

  public isTinted(): boolean {
    return this.tintIndex !== -1;
  }

  public getTintIndex(): number {
    return this.tintIndex;
  }

  public getDirection(): Direction {
    return this.direction;
  }

  public isShade(): boolean {
    return this.shade;
  }
}
