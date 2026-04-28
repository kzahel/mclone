import type { BlockPos } from "../../../../core/block-pos";
import { Direction } from "../../../../core/direction";
import { getSeed } from "../../../../util/mth";
import type { BlockGetter } from "../../block-getter";
import type { WorldGenLevel } from "../../world-gen-level";
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
import { Vec3 } from "../../../phys/vec3";
import { Shapes, type VoxelShape } from "../../../phys/shapes/voxel-shape";
import type { FluidState } from "../../material/fluid-state";
import { BlockTag } from "../../../../tags/block-tags";
import type { PathComputationType } from "../../pathfinder/path-computation-type";

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

  public skipRendering(_state: BlockState, _adjacentState: BlockState, _direction: Direction): boolean {
    return false;
  }

  public updateShape(
    state: BlockState,
    _direction: Direction,
    _neighborState: BlockState,
    _level: WorldGenLevel,
    _pos: BlockPos,
    _neighborPos: BlockPos,
  ): BlockState {
    return state;
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

  public getSeed(_state: BlockState, pos: BlockPos): bigint {
    return getSeed(pos);
  }

  public getLightBlock(state: BlockState, level: BlockGetter, pos: BlockPos): number {
    return state.isSolidRender(level, pos) ? level.getMaxLightLevel() : (state.propagatesSkylightDown(level, pos) ? 0 : 1);
  }

  public getShadeBrightness(state: BlockState, level: BlockGetter, pos: BlockPos): number {
    return state.isCollisionShapeFullBlock(level, pos) ? 0.2 : 1.0;
  }

  public getShape(_state: BlockState, _level: BlockGetter, _pos: BlockPos): VoxelShape {
    return Shapes.block();
  }

  public getCollisionShape(state: BlockState, level: BlockGetter, pos: BlockPos): VoxelShape {
    return this.hasCollision ? state.getShape(level, pos) : Shapes.empty();
  }

  public getOcclusionShape(state: BlockState, level: BlockGetter, pos: BlockPos): VoxelShape {
    return state.getShape(level, pos);
  }

  public getBlockSupportShape(state: BlockState, level: BlockGetter, pos: BlockPos): VoxelShape {
    return this.getCollisionShape(state, level, pos);
  }

  public isCollisionShapeFullBlock(state: BlockState, level: BlockGetter, pos: BlockPos): boolean {
    return state.getCollisionShape(level, pos).isFullBlock();
  }

  public propagatesSkylightDown(state: BlockState, _level: BlockGetter, _pos: BlockPos): boolean {
    return !state.canOcclude();
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
    private readonly isViewBlockingValue: boolean;
    private readonly emissiveRenderingValue: boolean;

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
      this.isViewBlockingValue = this.canOccludeValue;
      this.emissiveRenderingValue = false;
    }

    protected asState(): BlockState {
      return this as unknown as BlockState;
    }

    public getBlock(): Block {
      return this.owner;
    }

    public is(block: Block): boolean;
    public is(tag: BlockTag): boolean;
    public is(blockOrTag: Block | BlockTag): boolean {
      return blockOrTag instanceof BlockTag ? blockOrTag.contains(this.getBlock()) : this.getBlock() === blockOrTag;
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

    public getFluidState(): FluidState {
      return this.getBlock().getFluidState(this.asState());
    }

    public updateShape(direction: Direction, neighborState: BlockState, level: WorldGenLevel, pos: BlockPos, neighborPos: BlockPos): BlockState {
      return this.getBlock().updateShape(this.asState(), direction, neighborState, level, pos, neighborPos);
    }

    public propagatesSkylightDown(level: BlockGetter, pos: BlockPos): boolean {
      return this.getBlock().propagatesSkylightDown(this.asState(), level, pos);
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

    public emissiveRendering(_level: BlockGetter, _pos: BlockPos): boolean {
      return this.emissiveRenderingValue;
    }

    public getShadeBrightness(level: BlockGetter, pos: BlockPos): number {
      return this.getBlock().getShadeBrightness(this.asState(), level, pos);
    }

    public getDestroySpeed(_level: BlockGetter, _pos: BlockPos): number {
      return this.destroySpeed;
    }

    public canSurvive(level: WorldGenLevel, pos: BlockPos): boolean {
      return this.getBlock().canSurvive(this.asState(), level, pos);
    }

    public canOcclude(): boolean {
      return this.canOccludeValue;
    }

    public skipRendering(adjacentState: BlockState, direction: Direction): boolean {
      return this.getBlock().skipRendering(this.asState(), adjacentState, direction);
    }

    public useShapeForLightOcclusion(): boolean {
      return this.useShapeForLightOcclusionValue;
    }

    public requiresCorrectToolForDrops(): boolean {
      return this.requiresCorrectToolForDropsValue;
    }

    public isFaceSturdy(level: BlockGetter, pos: BlockPos, _direction: Direction): boolean {
      return this.isCollisionShapeFullBlock(level, pos);
    }

    public getLightBlock(level: BlockGetter, pos: BlockPos): number {
      return this.getBlock().getLightBlock(this.asState(), level, pos);
    }

    public isSolidRender(level: BlockGetter, _pos: BlockPos): boolean {
      return this.getBlock().isSolidRender(this.asState(), level);
    }

    public isViewBlocking(_level: BlockGetter, _pos: BlockPos): boolean {
      return this.isViewBlockingValue;
    }

    public getOffset(_level: BlockGetter, _pos: BlockPos): Vec3 {
      return Vec3.ZERO;
    }

    public getSeed(pos: BlockPos): bigint {
      return this.getBlock().getSeed(this.asState(), pos);
    }

    public getShape(level: BlockGetter, pos: BlockPos): VoxelShape {
      return this.getBlock().getShape(this.asState(), level, pos);
    }

    public getCollisionShape(level: BlockGetter, pos: BlockPos): VoxelShape {
      return this.getBlock().getCollisionShape(this.asState(), level, pos);
    }

    public getOcclusionShape(level: BlockGetter, pos: BlockPos): VoxelShape {
      return this.getBlock().getOcclusionShape(this.asState(), level, pos);
    }

    public getBlockSupportShape(level: BlockGetter, pos: BlockPos): VoxelShape {
      return this.getBlock().getBlockSupportShape(this.asState(), level, pos);
    }

    public isCollisionShapeFullBlock(level: BlockGetter, pos: BlockPos): boolean {
      return this.getBlock().isCollisionShapeFullBlock(this.asState(), level, pos);
    }

    public isPathfindable(level: BlockGetter, pos: BlockPos, type: PathComputationType): boolean {
      return this.getBlock().isPathfindable(this.asState(), level, pos, type);
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
