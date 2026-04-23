import { Direction } from "../../../../../core/direction";
import { BooleanProperty } from "./boolean-property";
import { DoubleBlockHalf } from "./double-block-half";
import { DirectionProperty } from "./direction-property";
import { EnumProperty } from "./enum-property";
import { IntegerProperty } from "./integer-property";

export class BlockStateProperties {
  public static readonly AGE_2 = IntegerProperty.create("age", 0, 2);
  public static readonly AGE_3 = IntegerProperty.create("age", 0, 3);
  public static readonly AGE_15 = IntegerProperty.create("age", 0, 15);
  public static readonly DISTANCE = IntegerProperty.create("distance", 1, 7);
  public static readonly LAYERS = IntegerProperty.create("layers", 1, 8);
  public static readonly LIT = BooleanProperty.create("lit");
  public static readonly LEVEL = IntegerProperty.create("level", 0, 15);
  public static readonly OPEN = BooleanProperty.create("open");
  public static readonly PICKLES = IntegerProperty.create("pickles", 1, 4);
  public static readonly PERSISTENT = BooleanProperty.create("persistent");
  public static readonly POWERED = BooleanProperty.create("powered");
  public static readonly SNOWY = BooleanProperty.create("snowy");
  public static readonly NORTH = BooleanProperty.create("north");
  public static readonly EAST = BooleanProperty.create("east");
  public static readonly SOUTH = BooleanProperty.create("south");
  public static readonly WEST = BooleanProperty.create("west");
  public static readonly UP = BooleanProperty.create("up");
  public static readonly DOWN = BooleanProperty.create("down");
  public static readonly AXIS = EnumProperty.create("axis", Direction.Axis);
  public static readonly DOUBLE_BLOCK_HALF = EnumProperty.create("half", DoubleBlockHalf);
  public static readonly HORIZONTAL_AXIS = EnumProperty.create("axis", Direction.Axis, Direction.Axis.X, Direction.Axis.Z);
  public static readonly WATERLOGGED = BooleanProperty.create("waterlogged");
  public static readonly FACING = DirectionProperty.create(
    "facing",
    Direction.NORTH,
    Direction.EAST,
    Direction.SOUTH,
    Direction.WEST,
    Direction.UP,
    Direction.DOWN,
  );
  public static readonly HORIZONTAL_FACING = DirectionProperty.create("facing", Direction.Plane.HORIZONTAL);
}
