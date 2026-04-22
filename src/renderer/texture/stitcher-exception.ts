import type { TextureAtlasSpriteInfo } from "./texture-atlas-sprite";

export class StitcherException extends Error {
  public constructor(
    sprite: TextureAtlasSpriteInfo,
    private readonly allSprites: readonly TextureAtlasSpriteInfo[],
  ) {
    super(`Unable to fit: ${sprite.name()} - size: ${sprite.width()}x${sprite.height()} - Maybe try a lower resolution resourcepack?`);
    this.name = "StitcherException";
  }

  public getAllSprites(): readonly TextureAtlasSpriteInfo[] {
    return this.allSprites;
  }
}
