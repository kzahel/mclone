import { SectionPos } from "../../../core/section-pos";
import { AABB } from "../../phys/aabb";
import type { RuntimeEntityAccess } from "./entity-access";
import { EntitySection } from "./entity-section";
import {
  entityChunkKey,
  entityChunkKeyFromSectionKey,
  parseEntityChunkKey,
  type EntityChunkPos,
} from "./chunk-entities";
import { isVisibilityAccessible, type Visibility } from "./visibility";

function intersects(left: AABB, right: AABB): boolean {
  return left.minX < right.maxX
    && left.maxX > right.minX
    && left.minY < right.maxY
    && left.maxY > right.minY
    && left.minZ < right.maxZ
    && left.maxZ > right.minZ;
}

export class EntitySectionStorage<T extends RuntimeEntityAccess> {
  private readonly sections = new Map<bigint, EntitySection<T>>();

  public constructor(private readonly initialSectionVisibility: (chunkX: number, chunkZ: number) => Visibility) {}

  public forEachAccessibleSection(box: AABB, consumer: (section: EntitySection<T>) => void): void {
    const minSectionX = SectionPos.posToSectionCoord(box.minX - 2.0);
    const minSectionY = SectionPos.posToSectionCoord(box.minY - 2.0);
    const minSectionZ = SectionPos.posToSectionCoord(box.minZ - 2.0);
    const maxSectionX = SectionPos.posToSectionCoord(box.maxX + 2.0);
    const maxSectionY = SectionPos.posToSectionCoord(box.maxY + 2.0);
    const maxSectionZ = SectionPos.posToSectionCoord(box.maxZ + 2.0);

    for (let sectionX = minSectionX; sectionX <= maxSectionX; sectionX++) {
      for (let sectionY = minSectionY; sectionY <= maxSectionY; sectionY++) {
        for (let sectionZ = minSectionZ; sectionZ <= maxSectionZ; sectionZ++) {
          const section = this.sections.get(SectionPos.asLong(sectionX, sectionY, sectionZ));
          if (section !== undefined && isVisibilityAccessible(section.getStatus())) {
            consumer(section);
          }
        }
      }
    }
  }

  public getExistingSectionPositionsInChunk(chunkX: number, chunkZ: number): readonly bigint[] {
    const positions: bigint[] = [];
    for (const sectionKey of this.sections.keys()) {
      if (SectionPos.x(sectionKey) === chunkX && SectionPos.z(sectionKey) === chunkZ) {
        positions.push(sectionKey);
      }
    }

    return positions.sort((left, right) => Number(left - right));
  }

  public getExistingSectionsInChunk(chunkX: number, chunkZ: number): readonly EntitySection<T>[] {
    return this.getExistingSectionPositionsInChunk(chunkX, chunkZ)
      .map((sectionKey) => this.sections.get(sectionKey))
      .filter((section): section is EntitySection<T> => section !== undefined);
  }

  public getOrCreateSection(sectionKey: bigint): EntitySection<T> {
    let section = this.sections.get(sectionKey);
    if (section === undefined) {
      const chunk = parseEntityChunkKey(entityChunkKeyFromSectionKey(sectionKey));
      section = new EntitySection<T>(this.initialSectionVisibility(chunk.chunkX, chunk.chunkZ));
      this.sections.set(sectionKey, section);
    }

    return section;
  }

  public getSection(sectionKey: bigint): EntitySection<T> | undefined {
    return this.sections.get(sectionKey);
  }

  public getAllChunksWithExistingSections(): readonly EntityChunkPos[] {
    const chunks = new Map<string, EntityChunkPos>();
    for (const sectionKey of this.sections.keys()) {
      const chunkX = SectionPos.x(sectionKey);
      const chunkZ = SectionPos.z(sectionKey);
      chunks.set(entityChunkKey(chunkX, chunkZ), { chunkX, chunkZ });
    }

    return [...chunks.values()].sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  public getEntities(box: AABB, consumer: (entity: T) => void): void {
    this.forEachAccessibleSection(box, (section) => {
      section.forEachEntity((entity) => intersects(entity.getBoundingBox(), box), consumer);
    });
  }

  public remove(sectionKey: bigint): void {
    this.sections.delete(sectionKey);
  }

  public count(): number {
    return this.sections.size;
  }
}
