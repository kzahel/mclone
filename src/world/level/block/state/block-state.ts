import type { Block } from "../block";
import { BlockBehaviour } from "./block-behaviour";
import type { Property } from "./properties/property";

export class BlockState extends BlockBehaviour.BlockStateBase {
  public constructor(block: Block, values: ReadonlyMap<Property<unknown>, unknown>) {
    super(block, values);
  }
}
