export class SoundType {
  public static readonly WOOD = new SoundType(1.0, 1.0);
  public static readonly GRAVEL = new SoundType(1.0, 1.0);
  public static readonly GRASS = new SoundType(1.0, 1.0);
  public static readonly STONE = new SoundType(1.0, 1.0);

  public constructor(
    public readonly volume: number,
    public readonly pitch: number,
  ) {}
}
