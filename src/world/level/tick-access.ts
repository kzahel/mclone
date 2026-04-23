import type { BlockPos } from "../../core/block-pos";

export interface TickAccess<T> {
  scheduleTick(pos: BlockPos, target: T, delay: number): void;
}

export class BlackholeTickAccess<T> implements TickAccess<T> {
  public scheduleTick(_pos: BlockPos, _target: T, _delay: number): void {}
}
