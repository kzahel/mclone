import { ResourceLocation } from "../../../core/resource-location";
import { BlockPos } from "../../../core/block-pos";
import { Direction } from "../../../core/direction";
import type { BlockPlaceContext } from "../../item/context/block-place-context";
import type { BlockGetter } from "../block-getter";
import { RenderShape } from "./render-shape";
import { SoundType } from "./sound-type";
import { BlockBehaviour } from "./state/block-behaviour";
import { BlockState } from "./state/block-state";
import { StateDefinition } from "./state/state-definition";
import type { Property } from "./state/properties/property";
import { Fluids } from "../material/fluids";
import type { FluidState } from "../material/fluid-state";

function copyProperty<T>(from: BlockState, to: BlockState, property: Property<T>): BlockState {
  return to.setValue(property, from.getValue(property));
}

export class Block extends BlockBehaviour {
  protected readonly stateDefinition: StateDefinition<Block, BlockState>;
  private defaultBlockStateValue: BlockState;
  private locationValue: ResourceLocation | undefined;

  public constructor(properties: BlockBehaviour.Properties) {
    super(properties);
    const builder = new StateDefinition.Builder<Block, BlockState>(this);
    this.createBlockStateDefinition(builder);
    this.stateDefinition = builder.create((owner) => owner.defaultBlockState(), {
      create: (owner, values) => new BlockState(owner, values),
    });
    this.defaultBlockStateValue = this.stateDefinition.any();
    this.registerDefaultState(this.stateDefinition.any());
  }

  protected createBlockStateDefinition(_builder: StateDefinition.Builder<Block, BlockState>): void {}

  public getStateDefinition(): StateDefinition<Block, BlockState> {
    return this.stateDefinition;
  }

  protected registerDefaultState(state: BlockState): void {
    this.defaultBlockStateValue = state;
  }

  public defaultBlockState(): BlockState {
    return this.defaultBlockStateValue;
  }

  public static shouldRenderFace(state: BlockState, level: BlockGetter, _pos: BlockPos, direction: Direction, neighborPos: BlockPos): boolean {
    const adjacentState = level.getBlockState(neighborPos);
    if (state.skipRendering(adjacentState, direction)) {
      return false;
    }

    return !adjacentState.canOcclude();
  }

  public withPropertiesOf(state: BlockState): BlockState {
    let result = this.defaultBlockState();
    for (const property of state.getBlock().getStateDefinition().getProperties()) {
      if (result.hasProperty(property)) {
        result = copyProperty(state, result, property as Property<unknown>);
      }
    }

    return result;
  }

  public override getSoundType(state: BlockState): SoundType {
    return super.getSoundType(state);
  }

  public getStateForPlacement(_context: BlockPlaceContext): BlockState | undefined {
    return this.defaultBlockState();
  }

  public useShapeForLightOcclusion(_state: BlockState): boolean {
    return false;
  }

  public getFluidState(_state: BlockState): FluidState {
    return Fluids.EMPTY.defaultFluidState();
  }

  public setLocation(location: ResourceLocation): this {
    this.locationValue = location;
    return this;
  }

  public getLocation(): ResourceLocation | undefined {
    return this.locationValue;
  }

  public override hasDynamicShape(): boolean {
    return this.dynamicShape;
  }

  public override toString(): string {
    return this.locationValue ? `Block{${this.locationValue}}` : `Block{${this.constructor.name}}`;
  }

  protected override asBlock(): Block {
    return this;
  }

  public override getRenderShape(_state: BlockState): RenderShape {
    return RenderShape.MODEL;
  }

  public canBeReplaced(_state: BlockState, _context: BlockPlaceContext): boolean {
    return this.material.isReplaceable();
  }

  public isSolidRender(state: BlockState, _level: BlockGetter): boolean {
    return state.canOcclude();
  }
}
