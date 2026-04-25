import type { PlayerMoveCommand } from "./movement-command";
import { validatePlayerMoveCommand } from "./movement-command";

export class MovementCommandBuffer {
  private readonly commands: PlayerMoveCommand[] = [];
  private lastStoredSequence = 0;

  public constructor(private readonly capacity = 128) {
    if (!Number.isSafeInteger(capacity) || capacity <= 0) {
      throw new Error(`movement command buffer capacity must be a positive safe integer, got ${capacity}`);
    }
  }

  public add(command: PlayerMoveCommand): void {
    validatePlayerMoveCommand(command);
    if (command.sequence <= this.lastStoredSequence) {
      throw new Error(`command sequence ${command.sequence} must be greater than previous sequence ${this.lastStoredSequence}`);
    }

    this.commands.push(command);
    this.lastStoredSequence = command.sequence;
    while (this.commands.length > this.capacity) {
      this.commands.shift();
    }
  }

  public dropAcknowledged(lastProcessedCommandSeq: number): void {
    while (this.commands.length > 0 && this.commands[0]!.sequence <= lastProcessedCommandSeq) {
      this.commands.shift();
    }
  }

  public clear(): void {
    this.commands.length = 0;
    this.lastStoredSequence = 0;
  }

  public get size(): number {
    return this.commands.length;
  }

  public get firstSequence(): number | undefined {
    return this.commands[0]?.sequence;
  }

  public get lastSequence(): number | undefined {
    return this.commands[this.commands.length - 1]?.sequence;
  }

  public getUnacknowledged(): readonly PlayerMoveCommand[] {
    return this.commands.slice();
  }
}
