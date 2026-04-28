import type { StructureProcessor } from "./structure-processor";

export class StructureProcessorList {
  public constructor(private readonly processors: readonly StructureProcessor[]) {}

  public list(): readonly StructureProcessor[] {
    return this.processors;
  }
}
