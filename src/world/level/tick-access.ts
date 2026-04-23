import type { BlockPos } from "../../core/block-pos";

export interface TickAccess<T> {
  scheduleTick(pos: BlockPos, target: T, delay: number): void;
}

export class RecordingTickAccess<T> implements TickAccess<T> {
  public constructor(private readonly schedule: (pos: BlockPos, target: T, delay: number) => void) {}

  public scheduleTick(pos: BlockPos, target: T, delay: number): void {
    this.schedule(pos, target, delay);
  }
}

export class BlackholeTickAccess<T> implements TickAccess<T> {
  public scheduleTick(_pos: BlockPos, _target: T, _delay: number): void {}
}
