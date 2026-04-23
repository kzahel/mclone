import { Fluid } from "./fluid";
import type { FluidState } from "./fluid-state";

class EmptyFluid extends Fluid {
  public constructor() {
    super();
  }

  public override isEmpty(): boolean {
    return true;
  }

  public override isSource(_state: FluidState): boolean {
    return false;
  }

  public override getAmount(_state: FluidState): number {
    return 0;
  }

  public override getOwnHeight(_state: FluidState): number {
    return 0.0;
  }
}

class SourceFluid extends Fluid {
  public constructor() {
    super();
  }
}

export class Fluids {
  public static readonly EMPTY = new EmptyFluid();
  public static readonly WATER = new SourceFluid();
  public static readonly LAVA = new SourceFluid();
}
