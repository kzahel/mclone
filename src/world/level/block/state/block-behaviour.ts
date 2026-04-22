import type { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import type { BlockGetter } from "../../block-getter";
import type { Block } from "../block";
import { type Mirror } from "../mirror";
import { RenderShape } from "../render-shape";
import { type Rotation } from "../rotation";
import { SoundType } from "../sound-type";
import { Material } from "../../material/material";
import { MaterialColor } from "../../material/material-color";
import { StateHolder } from "./state-holder";
import type { BlockState } from "./block-state";
import type { Property } from "./properties/property";

export abstract class BlockBehaviour {
  public readonly material: Material;
  public readonly hasCollision: boolean;
  public readonly explosionResistance: number;
  public readonly isRandomlyTicking: boolean;
  public readonly soundType: SoundType;
  public readonly friction: number;
  public readonly speedFactor: number;
  public readonly jumpFactor: number;
  public readonly dynamicShape: boolean;
  public readonly properties: BlockBehaviour.Properties;

  protected constructor(properties: BlockBehaviour.Properties) {
    this.material = properties.material;
    this.hasCollision = properties.hasCollision;
    this.explosionResistance = properties.explosionResistance;
    this.isRandomlyTicking = properties.isRandomlyTicking;
    this.soundType = properties.soundType;
    this.friction = properties.frictionValue;
    this.speedFactor = properties.speedFactorValue;
    this.jumpFactor = properties.jumpFactorValue;
    this.dynamicShape = properties.dynamicShapeValue;
    this.properties = properties;
  }

  public getRenderShape(_state: BlockState): RenderShape {
    return RenderShape.MODEL;
  }

  public rotate(state: BlockState, _rotation: Rotation): BlockState {
    return state;
  }

  public mirror(state: BlockState, _mirror: Mirror): BlockState {
    return state;
  }

  public defaultMaterialColor(): MaterialColor {
    return this.properties.materialColor(this.asBlock().defaultBlockState());
  }

  public defaultDestroyTime(): number {
    return this.properties.destroyTime;
  }

  public getSoundType(_state: BlockState): SoundType {
    return this.soundType;
  }

  public abstract hasDynamicShape(): boolean;

  protected abstract asBlock(): Block;
}

export namespace BlockBehaviour {
  export class BlockStateBase extends StateHolder<Block, BlockState> {
    private readonly lightEmission: number;
    private readonly useShapeForLightOcclusionValue: boolean;
    private readonly air: boolean;
    private readonly material: Material;
    private readonly materialColor: MaterialColor;
    private readonly destroySpeed: number;
    private readonly requiresCorrectToolForDropsValue: boolean;
    private readonly canOccludeValue: boolean;

    protected constructor(block: Block, values: ReadonlyMap<Property<unknown>, unknown>) {
      super(block, values);
      const properties = block.properties;
      this.lightEmission = properties.lightEmission(this.asState());
      this.useShapeForLightOcclusionValue = block.useShapeForLightOcclusion(this.asState());
      this.air = properties.isAir;
      this.material = properties.material;
      this.materialColor = properties.materialColor(this.asState());
      this.destroySpeed = properties.destroyTime;
      this.requiresCorrectToolForDropsValue = properties.requiresCorrectToolForDropsValue;
      this.canOccludeValue = properties.canOcclude;
    }

    protected asState(): BlockState {
      return this as unknown as BlockState;
    }

    public getBlock(): Block {
      return this.owner;
    }

    public getMaterial(): Material {
      return this.material;
    }

    public getLightEmission(): number {
      return this.lightEmission;
    }

    public isAir(): boolean {
      return this.air;
    }

    public getMapColor(_level: BlockGetter, _pos: BlockPos): MaterialColor {
      return this.materialColor;
    }

    public rotate(rotation: Rotation): BlockState {
      return this.getBlock().rotate(this.asState(), rotation);
    }

    public mirror(mirror: Mirror): BlockState {
      return this.getBlock().mirror(this.asState(), mirror);
    }

    public getRenderShape(): RenderShape {
      return this.getBlock().getRenderShape(this.asState());
    }

    public getDestroySpeed(_level: BlockGetter, _pos: BlockPos): number {
      return this.destroySpeed;
    }

    public canOcclude(): boolean {
      return this.canOccludeValue;
    }

    public useShapeForLightOcclusion(): boolean {
      return this.useShapeForLightOcclusionValue;
    }

    public requiresCorrectToolForDrops(): boolean {
      return this.requiresCorrectToolForDropsValue;
    }

    public isFaceSturdy(_level: BlockGetter, _pos: BlockPos, direction: Direction): boolean {
      return direction === Direction.UP && this.canOcclude();
    }
  }

  export class Properties {
    public material: Material;
    public materialColor: (state: BlockState) => MaterialColor;
    public hasCollision = true;
    public soundType = SoundType.STONE;
    public lightEmission = (_state: BlockState): number => 0;
    public explosionResistance = 0;
    public destroyTime = 0;
    public requiresCorrectToolForDropsValue = false;
    public isRandomlyTicking = false;
    public frictionValue = 0.6;
    public speedFactorValue = 1.0;
    public jumpFactorValue = 1.0;
    public canOcclude = true;
    public isAir = false;
    public dynamicShapeValue = false;

    private constructor(material: Material, materialColor: MaterialColor | ((state: BlockState) => MaterialColor)) {
      this.material = material;
      this.materialColor = typeof materialColor === "function" ? materialColor : () => materialColor;
    }

    public static of(material: Material): Properties;
    public static of(material: Material, materialColor: MaterialColor): Properties;
    public static of(material: Material, materialColor: (state: BlockState) => MaterialColor): Properties;
    public static of(material: Material, materialColor?: MaterialColor | ((state: BlockState) => MaterialColor)): Properties {
      return new Properties(material, materialColor ?? material.getColor());
    }

    public static copy(block: BlockBehaviour): Properties {
      const properties = new Properties(block.material, block.properties.materialColor);
      properties.material = block.properties.material;
      properties.destroyTime = block.properties.destroyTime;
      properties.explosionResistance = block.properties.explosionResistance;
      properties.hasCollision = block.properties.hasCollision;
      properties.isRandomlyTicking = block.properties.isRandomlyTicking;
      properties.lightEmission = block.properties.lightEmission;
      properties.materialColor = block.properties.materialColor;
      properties.soundType = block.properties.soundType;
      properties.frictionValue = block.properties.frictionValue;
      properties.speedFactorValue = block.properties.speedFactorValue;
      properties.dynamicShapeValue = block.properties.dynamicShapeValue;
      properties.canOcclude = block.properties.canOcclude;
      properties.isAir = block.properties.isAir;
      properties.requiresCorrectToolForDropsValue = block.properties.requiresCorrectToolForDropsValue;
      return properties;
    }

    public noCollission(): Properties {
      this.hasCollision = false;
      this.canOcclude = false;
      return this;
    }

    public noOcclusion(): Properties {
      this.canOcclude = false;
      return this;
    }

    public friction(value: number): Properties {
      this.frictionValue = value;
      return this;
    }

    public speedFactor(value: number): Properties {
      this.speedFactorValue = value;
      return this;
    }

    public jumpFactor(value: number): Properties {
      this.jumpFactorValue = value;
      return this;
    }

    public sound(value: SoundType): Properties {
      this.soundType = value;
      return this;
    }

    public lightLevel(value: (state: BlockState) => number): Properties {
      this.lightEmission = value;
      return this;
    }

    public strength(destroyTime: number, explosionResistance: number = destroyTime): Properties {
      return this.setDestroyTime(destroyTime).setExplosionResistance(explosionResistance);
    }

    public instabreak(): Properties {
      return this.strength(0.0);
    }

    public randomTicks(): Properties {
      this.isRandomlyTicking = true;
      return this;
    }

    public dynamicShape(): Properties {
      this.dynamicShapeValue = true;
      return this;
    }

    public air(): Properties {
      this.isAir = true;
      return this;
    }

    public requiresCorrectToolForDrops(): Properties {
      this.requiresCorrectToolForDropsValue = true;
      return this;
    }

    public color(value: MaterialColor): Properties {
      this.materialColor = () => value;
      return this;
    }

    public setDestroyTime(value: number): Properties {
      this.destroyTime = value;
      return this;
    }

    public setExplosionResistance(value: number): Properties {
      this.explosionResistance = Math.max(0.0, value);
      return this;
    }
  }
}
