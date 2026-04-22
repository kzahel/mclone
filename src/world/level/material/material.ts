import { MaterialColor } from "./material-color";
import { PushReaction } from "./push-reaction";

class MaterialBuilder {
  private pushReaction = PushReaction.NORMAL;
  private blocksMotion = true;
  private flammableValue = false;
  private liquidValue = false;
  private replaceableValue = false;
  private solidValue = true;
  private solidBlockingValue = true;

  public constructor(private readonly color: MaterialColor) {}

  public liquid(): MaterialBuilder {
    this.liquidValue = true;
    return this;
  }

  public nonSolid(): MaterialBuilder {
    this.solidValue = false;
    return this;
  }

  public noCollider(): MaterialBuilder {
    this.blocksMotion = false;
    return this;
  }

  public notSolidBlocking(): MaterialBuilder {
    this.solidBlockingValue = false;
    return this;
  }

  public flammable(): MaterialBuilder {
    this.flammableValue = true;
    return this;
  }

  public replaceable(): MaterialBuilder {
    this.replaceableValue = true;
    return this;
  }

  public destroyOnPush(): MaterialBuilder {
    this.pushReaction = PushReaction.DESTROY;
    return this;
  }

  public notPushable(): MaterialBuilder {
    this.pushReaction = PushReaction.BLOCK;
    return this;
  }

  public build(factory: typeof Material): Material {
    return new factory(
      this.color,
      this.liquidValue,
      this.solidValue,
      this.blocksMotion,
      this.solidBlockingValue,
      this.flammableValue,
      this.replaceableValue,
      this.pushReaction,
    );
  }
}

export class Material {
  public static AIR: Material;
  public static STRUCTURAL_AIR: Material;
  public static PORTAL: Material;
  public static CLOTH_DECORATION: Material;
  public static PLANT: Material;
  public static WATER_PLANT: Material;
  public static REPLACEABLE_PLANT: Material;
  public static REPLACEABLE_FIREPROOF_PLANT: Material;
  public static REPLACEABLE_WATER_PLANT: Material;
  public static WATER: Material;
  public static BUBBLE_COLUMN: Material;
  public static LAVA: Material;
  public static TOP_SNOW: Material;
  public static FIRE: Material;
  public static DECORATION: Material;
  public static WEB: Material;
  public static SCULK: Material;
  public static BUILDABLE_GLASS: Material;
  public static CLAY: Material;
  public static DIRT: Material;
  public static GRASS: Material;
  public static ICE_SOLID: Material;
  public static SAND: Material;
  public static SPONGE: Material;
  public static SHULKER_SHELL: Material;
  public static WOOD: Material;
  public static NETHER_WOOD: Material;
  public static BAMBOO_SAPLING: Material;
  public static BAMBOO: Material;
  public static WOOL: Material;
  public static EXPLOSIVE: Material;
  public static LEAVES: Material;
  public static GLASS: Material;
  public static ICE: Material;
  public static CACTUS: Material;
  public static STONE: Material;
  public static METAL: Material;
  public static SNOW: Material;
  public static HEAVY_METAL: Material;
  public static BARRIER: Material;
  public static PISTON: Material;
  public static MOSS: Material;
  public static VEGETABLE: Material;
  public static EGG: Material;
  public static CAKE: Material;
  public static AMETHYST: Material;
  public static POWDER_SNOW: Material;

  static {
    this.AIR = new MaterialBuilder(MaterialColor.NONE).noCollider().notSolidBlocking().nonSolid().replaceable().build(this);
    this.STRUCTURAL_AIR = new MaterialBuilder(MaterialColor.NONE).noCollider().notSolidBlocking().nonSolid().replaceable().build(this);
    this.PORTAL = new MaterialBuilder(MaterialColor.NONE).noCollider().notSolidBlocking().nonSolid().notPushable().build(this);
    this.CLOTH_DECORATION = new MaterialBuilder(MaterialColor.WOOL).noCollider().notSolidBlocking().nonSolid().flammable().build(this);
    this.PLANT = new MaterialBuilder(MaterialColor.PLANT).noCollider().notSolidBlocking().nonSolid().destroyOnPush().build(this);
    this.WATER_PLANT = new MaterialBuilder(MaterialColor.WATER).noCollider().notSolidBlocking().nonSolid().destroyOnPush().build(this);
    this.REPLACEABLE_PLANT = new MaterialBuilder(MaterialColor.PLANT).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().flammable().build(this);
    this.REPLACEABLE_FIREPROOF_PLANT = new MaterialBuilder(MaterialColor.PLANT).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().build(this);
    this.REPLACEABLE_WATER_PLANT = new MaterialBuilder(MaterialColor.WATER).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().build(this);
    this.WATER = new MaterialBuilder(MaterialColor.WATER).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().liquid().build(this);
    this.BUBBLE_COLUMN = new MaterialBuilder(MaterialColor.WATER).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().liquid().build(this);
    this.LAVA = new MaterialBuilder(MaterialColor.FIRE).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().liquid().build(this);
    this.TOP_SNOW = new MaterialBuilder(MaterialColor.SNOW).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().build(this);
    this.FIRE = new MaterialBuilder(MaterialColor.NONE).noCollider().notSolidBlocking().nonSolid().destroyOnPush().replaceable().build(this);
    this.DECORATION = new MaterialBuilder(MaterialColor.NONE).noCollider().notSolidBlocking().nonSolid().destroyOnPush().build(this);
    this.WEB = new MaterialBuilder(MaterialColor.WOOL).noCollider().notSolidBlocking().destroyOnPush().build(this);
    this.SCULK = new MaterialBuilder(MaterialColor.COLOR_BLACK).build(this);
    this.BUILDABLE_GLASS = new MaterialBuilder(MaterialColor.NONE).build(this);
    this.CLAY = new MaterialBuilder(MaterialColor.CLAY).build(this);
    this.DIRT = new MaterialBuilder(MaterialColor.DIRT).build(this);
    this.GRASS = new MaterialBuilder(MaterialColor.GRASS).build(this);
    this.ICE_SOLID = new MaterialBuilder(MaterialColor.ICE).build(this);
    this.SAND = new MaterialBuilder(MaterialColor.SAND).build(this);
    this.SPONGE = new MaterialBuilder(MaterialColor.COLOR_YELLOW).build(this);
    this.SHULKER_SHELL = new MaterialBuilder(MaterialColor.COLOR_PURPLE).build(this);
    this.WOOD = new MaterialBuilder(MaterialColor.WOOD).flammable().build(this);
    this.NETHER_WOOD = new MaterialBuilder(MaterialColor.WOOD).build(this);
    this.BAMBOO_SAPLING = new MaterialBuilder(MaterialColor.WOOD).flammable().destroyOnPush().noCollider().build(this);
    this.BAMBOO = new MaterialBuilder(MaterialColor.WOOD).flammable().destroyOnPush().build(this);
    this.WOOL = new MaterialBuilder(MaterialColor.WOOL).flammable().build(this);
    this.EXPLOSIVE = new MaterialBuilder(MaterialColor.FIRE).flammable().notSolidBlocking().build(this);
    this.LEAVES = new MaterialBuilder(MaterialColor.PLANT).flammable().notSolidBlocking().destroyOnPush().build(this);
    this.GLASS = new MaterialBuilder(MaterialColor.NONE).notSolidBlocking().build(this);
    this.ICE = new MaterialBuilder(MaterialColor.ICE).notSolidBlocking().build(this);
    this.CACTUS = new MaterialBuilder(MaterialColor.PLANT).notSolidBlocking().destroyOnPush().build(this);
    this.STONE = new MaterialBuilder(MaterialColor.STONE).build(this);
    this.METAL = new MaterialBuilder(MaterialColor.METAL).build(this);
    this.SNOW = new MaterialBuilder(MaterialColor.SNOW).build(this);
    this.HEAVY_METAL = new MaterialBuilder(MaterialColor.METAL).notPushable().build(this);
    this.BARRIER = new MaterialBuilder(MaterialColor.NONE).notPushable().build(this);
    this.PISTON = new MaterialBuilder(MaterialColor.STONE).notPushable().build(this);
    this.MOSS = new MaterialBuilder(MaterialColor.PLANT).destroyOnPush().build(this);
    this.VEGETABLE = new MaterialBuilder(MaterialColor.PLANT).destroyOnPush().build(this);
    this.EGG = new MaterialBuilder(MaterialColor.PLANT).destroyOnPush().build(this);
    this.CAKE = new MaterialBuilder(MaterialColor.NONE).destroyOnPush().build(this);
    this.AMETHYST = new MaterialBuilder(MaterialColor.COLOR_PURPLE).build(this);
    this.POWDER_SNOW = new MaterialBuilder(MaterialColor.SNOW).nonSolid().noCollider().build(this);
  }

  public constructor(
    private readonly color: MaterialColor,
    private readonly liquid: boolean,
    private readonly solid: boolean,
    private readonly blocksMotionValue: boolean,
    private readonly solidBlocking: boolean,
    private readonly flammable: boolean,
    private readonly replaceable: boolean,
    private readonly pushReaction: PushReaction,
  ) {}

  public isLiquid(): boolean {
    return this.liquid;
  }

  public isSolid(): boolean {
    return this.solid;
  }

  public blocksMotion(): boolean {
    return this.blocksMotionValue;
  }

  public isFlammable(): boolean {
    return this.flammable;
  }

  public isReplaceable(): boolean {
    return this.replaceable;
  }

  public isSolidBlocking(): boolean {
    return this.solidBlocking;
  }

  public getPushReaction(): PushReaction {
    return this.pushReaction;
  }

  public getColor(): MaterialColor {
    return this.color;
  }
}
