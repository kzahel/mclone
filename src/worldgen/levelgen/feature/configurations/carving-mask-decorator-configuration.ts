import { GenerationStep } from "../../generation-step";
import type { DecoratorConfiguration } from "./decorator-configuration";

export class CarvingMaskDecoratorConfiguration implements DecoratorConfiguration {
  public constructor(
    public readonly step: GenerationStep.Carving,
  ) {}
}
