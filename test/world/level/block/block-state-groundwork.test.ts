import { describe, expect, test } from "vitest";
import { Direction } from "../../../../src/core/direction";
import { BlockPlaceContext } from "../../../../src/world/item/context/block-place-context";
import { Block } from "../../../../src/world/level/block/block";
import { HorizontalDirectionalBlock } from "../../../../src/world/level/block/horizontal-directional-block";
import { Mirror } from "../../../../src/world/level/block/mirror";
import { RotatedPillarBlock } from "../../../../src/world/level/block/rotated-pillar-block";
import { Rotation } from "../../../../src/world/level/block/rotation";
import { BlockBehaviour } from "../../../../src/world/level/block/state/block-behaviour";
import type { BlockState } from "../../../../src/world/level/block/state/block-state";
import { StateDefinition } from "../../../../src/world/level/block/state/state-definition";
import { BlockStateProperties } from "../../../../src/world/level/block/state/properties/block-state-properties";
import { BooleanProperty } from "../../../../src/world/level/block/state/properties/boolean-property";
import { EnumProperty } from "../../../../src/world/level/block/state/properties/enum-property";
import { IntegerProperty } from "../../../../src/world/level/block/state/properties/integer-property";
import { Material } from "../../../../src/world/level/material/material";

class ToggleBlock extends Block {
  public static readonly LIT = BlockStateProperties.LIT;

  public constructor() {
    super(BlockBehaviour.Properties.of(Material.STONE));
    this.registerDefaultState(this.defaultBlockState().setValue(ToggleBlock.LIT, false));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(ToggleBlock.LIT);
  }
}

class MirrorableBlock extends HorizontalDirectionalBlock {
  public constructor() {
    super(BlockBehaviour.Properties.of(Material.STONE));
    this.registerDefaultState(this.defaultBlockState().setValue(HorizontalDirectionalBlock.FACING, Direction.NORTH));
  }

  protected override createBlockStateDefinition(builder: StateDefinition.Builder<Block, BlockState>): void {
    builder.add(HorizontalDirectionalBlock.FACING);
  }
}

describe("Block state groundwork", () => {
  test("property implementations preserve Java-style serialized names and value sets", () => {
    const age = IntegerProperty.create("age", 0, 3);
    const axis = EnumProperty.create("axis", Direction.Axis);

    expect(BlockStateProperties.LIT.getValue("true")).toBe(true);
    expect(BlockStateProperties.LIT.getValue("false")).toBe(false);
    expect(BlockStateProperties.LIT.getNameForValue(false)).toBe("false");
    expect(age.getPossibleValues()).toEqual([0, 1, 2, 3]);
    expect(age.getValue("2")).toBe(2);
    expect(age.getValue("4")).toBeUndefined();
    expect(axis.getValue("z")).toBe(Direction.Axis.Z);
    expect(axis.getNameForValue(Direction.Axis.Y)).toBe("y");
    expect(BlockStateProperties.HORIZONTAL_FACING.getPossibleValues()).toEqual([
      Direction.NORTH,
      Direction.SOUTH,
      Direction.WEST,
      Direction.EAST,
    ]);
  });

  test("StateDefinition.Builder rejects invalid and duplicate property names", () => {
    const invalidBuilder = new StateDefinition.Builder<string, never>("test_block");
    expect(() => invalidBuilder.add(BooleanProperty.create("BadName"))).toThrow("invalidly named property");

    const duplicateBuilder = new StateDefinition.Builder<string, never>("test_block");
    duplicateBuilder.add(BooleanProperty.create("lit"));
    expect(() => duplicateBuilder.add(BooleanProperty.create("lit"))).toThrow("duplicate property: lit");
  });

  test("state holders cycle values and copy shared properties across blocks", () => {
    const sourceBlock = new ToggleBlock();
    const targetBlock = new ToggleBlock();
    const unlit = sourceBlock.defaultBlockState();
    const lit = unlit.cycle(ToggleBlock.LIT);

    expect(sourceBlock.getStateDefinition().getPossibleStates()).toHaveLength(2);
    expect(unlit.getValue(ToggleBlock.LIT)).toBe(false);
    expect(lit.getValue(ToggleBlock.LIT)).toBe(true);
    expect(lit.cycle(ToggleBlock.LIT)).toBe(unlit);
    expect(targetBlock.withPropertiesOf(lit).getValue(ToggleBlock.LIT)).toBe(true);
  });

  test("RotatedPillarBlock keeps Y as default and derives axis from placement face", () => {
    const block = new RotatedPillarBlock(BlockBehaviour.Properties.of(Material.WOOD));
    const xState = block.defaultBlockState().setValue(RotatedPillarBlock.AXIS, Direction.Axis.X);

    expect(block.defaultBlockState().getValue(RotatedPillarBlock.AXIS)).toBe(Direction.Axis.Y);
    expect(block.getStateForPlacement(new BlockPlaceContext(Direction.WEST))?.getValue(RotatedPillarBlock.AXIS)).toBe(Direction.Axis.X);
    expect(block.getStateForPlacement(new BlockPlaceContext(Direction.UP))?.getValue(RotatedPillarBlock.AXIS)).toBe(Direction.Axis.Y);
    expect(block.rotate(xState, Rotation.CLOCKWISE_90).getValue(RotatedPillarBlock.AXIS)).toBe(Direction.Axis.Z);
  });

  test("HorizontalDirectionalBlock rotates and mirrors the facing property", () => {
    const block = new MirrorableBlock();
    const north = block.defaultBlockState();
    const east = block.rotate(north, Rotation.CLOCKWISE_90);

    expect(north.getValue(HorizontalDirectionalBlock.FACING)).toBe(Direction.NORTH);
    expect(east.getValue(HorizontalDirectionalBlock.FACING)).toBe(Direction.EAST);
    expect(block.mirror(north, Mirror.LEFT_RIGHT).getValue(HorizontalDirectionalBlock.FACING)).toBe(Direction.SOUTH);
    expect(block.mirror(east, Mirror.FRONT_BACK).getValue(HorizontalDirectionalBlock.FACING)).toBe(Direction.WEST);
  });
});
