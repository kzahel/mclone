export interface MovementCommandClockOptions {
  readonly commandQuantumUs: number;
  readonly maxStepCountPerCommand?: number;
  readonly maxCatchupStepCount?: number;
}

export class MovementCommandClock {
  private accumulatedUs = 0;
  private readonly maxStepCountPerCommand: number;
  private readonly maxCatchupStepCount: number;

  public constructor(private readonly options: MovementCommandClockOptions) {
    if (!Number.isSafeInteger(options.commandQuantumUs) || options.commandQuantumUs <= 0) {
      throw new Error(`commandQuantumUs must be a positive safe integer, got ${options.commandQuantumUs}`);
    }

    this.maxStepCountPerCommand = options.maxStepCountPerCommand ?? 8;
    this.maxCatchupStepCount = options.maxCatchupStepCount ?? 24;
    if (!Number.isSafeInteger(this.maxStepCountPerCommand) || this.maxStepCountPerCommand <= 0) {
      throw new Error(`maxStepCountPerCommand must be a positive safe integer, got ${this.maxStepCountPerCommand}`);
    }
    if (!Number.isSafeInteger(this.maxCatchupStepCount) || this.maxCatchupStepCount <= 0) {
      throw new Error(`maxCatchupStepCount must be a positive safe integer, got ${this.maxCatchupStepCount}`);
    }
  }

  public get commandQuantumUs(): number {
    return this.options.commandQuantumUs;
  }

  public get pendingRemainderUs(): number {
    return this.accumulatedUs;
  }

  public consumeElapsedUs(elapsedUs: number): readonly number[] {
    if (!Number.isFinite(elapsedUs) || elapsedUs < 0.0) {
      throw new Error(`elapsedUs must be finite and non-negative, got ${elapsedUs}`);
    }

    this.accumulatedUs += elapsedUs;
    const availableStepCount = Math.floor(this.accumulatedUs / this.options.commandQuantumUs);
    if (availableStepCount <= 0) {
      return [];
    }

    const consumedStepCount = Math.min(availableStepCount, this.maxCatchupStepCount);
    this.accumulatedUs -= consumedStepCount * this.options.commandQuantumUs;
    if (availableStepCount > consumedStepCount) {
      this.accumulatedUs = 0;
    }

    return splitCommandStepCounts(consumedStepCount, this.maxStepCountPerCommand);
  }
}

export function splitCommandStepCounts(stepCount: number, maxStepCountPerCommand: number): readonly number[] {
  if (!Number.isSafeInteger(stepCount) || stepCount < 0) {
    throw new Error(`stepCount must be a non-negative safe integer, got ${stepCount}`);
  }
  if (!Number.isSafeInteger(maxStepCountPerCommand) || maxStepCountPerCommand <= 0) {
    throw new Error(`maxStepCountPerCommand must be a positive safe integer, got ${maxStepCountPerCommand}`);
  }

  const result: number[] = [];
  let remaining = stepCount;
  while (remaining > 0) {
    const current = Math.min(remaining, maxStepCountPerCommand);
    result.push(current);
    remaining -= current;
  }
  return result;
}
