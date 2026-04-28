export class SoundType {
  public static readonly WOOD = new SoundType(1.0, 1.0);
  public static readonly GRAVEL = new SoundType(1.0, 1.0);
  public static readonly GRASS = new SoundType(1.0, 1.0);
  public static readonly BAMBOO = new SoundType(1.0, 1.0);
  public static readonly BAMBOO_SAPLING = new SoundType(1.0, 1.0);
  public static readonly SNOW = new SoundType(1.0, 1.0);
  public static readonly STONE = new SoundType(1.0, 1.0);
  public static readonly BONE_BLOCK = new SoundType(1.0, 1.0);
  public static readonly DEEPSLATE = new SoundType(1.0, 1.0);
  public static readonly TUFF = new SoundType(1.0, 1.0);
  public static readonly CALCITE = new SoundType(1.0, 1.0);
  public static readonly GLOW_LICHEN = new SoundType(1.0, 1.0);
  public static readonly DRIPSTONE_BLOCK = new SoundType(1.0, 1.0);
  public static readonly POINTED_DRIPSTONE = new SoundType(1.0, 1.0);

  public constructor(
    public readonly volume: number,
    public readonly pitch: number,
  ) {}
}
