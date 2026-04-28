import { ChunkPos } from "../../core/chunk-pos";
import type { GeneratedRenderLevel } from "./generated-render-level";
import type { StructureFeature } from "../../worldgen/levelgen/structure/structure-feature";
import type { StructureStart } from "./levelgen/structure/structure-start";

export class StructureFeatureManager {
  public constructor(private readonly level: GeneratedRenderLevel) {}

  public startsForFeature(chunkX: number, chunkZ: number, structure: StructureFeature<any>): readonly StructureStart<any>[] {
    const starts: StructureStart<any>[] = [];
    for (const reference of this.level.getReferencesForFeature(chunkX, chunkZ, structure)) {
      const referencedChunkPos = ChunkPos.ofLong(reference);
      const start = this.level.getStartForFeature(referencedChunkPos.x, referencedChunkPos.z, structure);
      if (start !== undefined && start.isValid()) {
        starts.push(start);
      }
    }

    return starts;
  }

  public getStartForFeature(chunkX: number, chunkZ: number, structure: StructureFeature<any>): StructureStart<any> | undefined {
    return this.level.getStartForFeature(chunkX, chunkZ, structure);
  }

  public setStartForFeature(chunkX: number, chunkZ: number, structure: StructureFeature<any>, start: StructureStart<any> | undefined): void {
    this.level.setStartForFeature(chunkX, chunkZ, structure, start);
  }

  public addReferenceForFeature(chunkX: number, chunkZ: number, structure: StructureFeature<any>, reference: bigint): void {
    this.level.addReferenceForFeature(chunkX, chunkZ, structure, reference);
  }

  public getAllStarts(chunkX: number, chunkZ: number): ReadonlyMap<StructureFeature<any>, StructureStart<any>> {
    return this.level.getAllStarts(chunkX, chunkZ);
  }
}
