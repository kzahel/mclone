export class MaterialColor {
  public static readonly MATERIAL_COLORS = new Array<MaterialColor | undefined>(64);
  public static readonly NONE = new MaterialColor(0, 0);
  public static readonly GRASS = new MaterialColor(1, 8368696);
  public static readonly SAND = new MaterialColor(2, 16247203);
  public static readonly WOOL = new MaterialColor(3, 13092807);
  public static readonly FIRE = new MaterialColor(4, 16711680);
  public static readonly ICE = new MaterialColor(5, 10526975);
  public static readonly METAL = new MaterialColor(6, 10987431);
  public static readonly PLANT = new MaterialColor(7, 31744);
  public static readonly SNOW = new MaterialColor(8, 16777215);
  public static readonly CLAY = new MaterialColor(9, 10791096);
  public static readonly DIRT = new MaterialColor(10, 9923917);
  public static readonly STONE = new MaterialColor(11, 7368816);
  public static readonly WATER = new MaterialColor(12, 4210943);
  public static readonly WOOD = new MaterialColor(13, 9402184);
  public static readonly QUARTZ = new MaterialColor(14, 16776437);
  public static readonly COLOR_ORANGE = new MaterialColor(15, 14188339);
  public static readonly COLOR_MAGENTA = new MaterialColor(16, 11685080);
  public static readonly COLOR_LIGHT_BLUE = new MaterialColor(17, 6724056);
  public static readonly COLOR_YELLOW = new MaterialColor(18, 15066419);
  public static readonly COLOR_LIGHT_GREEN = new MaterialColor(19, 8375321);
  public static readonly COLOR_PINK = new MaterialColor(20, 15892389);
  public static readonly COLOR_GRAY = new MaterialColor(21, 5000268);
  public static readonly COLOR_LIGHT_GRAY = new MaterialColor(22, 10066329);
  public static readonly COLOR_CYAN = new MaterialColor(23, 5013401);
  public static readonly COLOR_PURPLE = new MaterialColor(24, 8339378);
  public static readonly COLOR_BLUE = new MaterialColor(25, 3361970);
  public static readonly COLOR_BROWN = new MaterialColor(26, 6704179);
  public static readonly COLOR_GREEN = new MaterialColor(27, 6717235);
  public static readonly COLOR_RED = new MaterialColor(28, 10040115);
  public static readonly COLOR_BLACK = new MaterialColor(29, 1644825);
  public static readonly GOLD = new MaterialColor(30, 16445005);
  public static readonly DIAMOND = new MaterialColor(31, 6085589);
  public static readonly LAPIS = new MaterialColor(32, 4882687);
  public static readonly EMERALD = new MaterialColor(33, 55610);
  public static readonly PODZOL = new MaterialColor(34, 8476209);
  public static readonly NETHER = new MaterialColor(35, 7340544);
  public static readonly TERRACOTTA_WHITE = new MaterialColor(36, 13742497);
  public static readonly TERRACOTTA_ORANGE = new MaterialColor(37, 10441252);
  public static readonly TERRACOTTA_MAGENTA = new MaterialColor(38, 9787244);
  public static readonly TERRACOTTA_LIGHT_BLUE = new MaterialColor(39, 7367818);
  public static readonly TERRACOTTA_YELLOW = new MaterialColor(40, 12223780);
  public static readonly TERRACOTTA_LIGHT_GREEN = new MaterialColor(41, 6780213);
  public static readonly TERRACOTTA_PINK = new MaterialColor(42, 10505550);
  public static readonly TERRACOTTA_GRAY = new MaterialColor(43, 3746083);
  public static readonly TERRACOTTA_LIGHT_GRAY = new MaterialColor(44, 8874850);
  public static readonly TERRACOTTA_CYAN = new MaterialColor(45, 5725276);
  public static readonly TERRACOTTA_PURPLE = new MaterialColor(46, 8014168);
  public static readonly TERRACOTTA_BLUE = new MaterialColor(47, 4996700);
  public static readonly TERRACOTTA_BROWN = new MaterialColor(48, 4993571);
  public static readonly TERRACOTTA_GREEN = new MaterialColor(49, 5001770);
  public static readonly TERRACOTTA_RED = new MaterialColor(50, 9321518);
  public static readonly TERRACOTTA_BLACK = new MaterialColor(51, 2430480);
  public static readonly CRIMSON_NYLIUM = new MaterialColor(52, 12398641);
  public static readonly CRIMSON_STEM = new MaterialColor(53, 9715553);
  public static readonly CRIMSON_HYPHAE = new MaterialColor(54, 6035741);
  public static readonly WARPED_NYLIUM = new MaterialColor(55, 1474182);
  public static readonly WARPED_STEM = new MaterialColor(56, 3837580);
  public static readonly WARPED_HYPHAE = new MaterialColor(57, 5647422);
  public static readonly WARPED_WART_BLOCK = new MaterialColor(58, 1356933);
  public static readonly DEEPSLATE = new MaterialColor(59, 6579300);
  public static readonly RAW_IRON = new MaterialColor(60, 14200723);
  public static readonly GLOW_LICHEN = new MaterialColor(61, 8365974);

  public constructor(
    public readonly id: number,
    public readonly col: number,
  ) {
    if (id < 0 || id > 63) {
      throw new Error("Map colour ID must be between 0 and 63 (inclusive)");
    }

    MaterialColor.MATERIAL_COLORS[id] = this;
  }

  public calculateRGBColor(brightness: number): number {
    let modifier = 220;
    if (brightness === 3) {
      modifier = 135;
    }

    if (brightness === 2) {
      modifier = 255;
    }

    if (brightness === 1) {
      modifier = 220;
    }

    if (brightness === 0) {
      modifier = 180;
    }

    const red = (((this.col >> 16) & 0xff) * modifier) / 255;
    const green = (((this.col >> 8) & 0xff) * modifier) / 255;
    const blue = ((this.col & 0xff) * modifier) / 255;
    return (0xff000000 | (Math.trunc(blue) << 16) | (Math.trunc(green) << 8) | Math.trunc(red)) >>> 0;
  }
}
