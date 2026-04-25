import { AABB } from "../../phys/aabb";
import type { RuntimeEntityAccess } from "./entity-access";
import type { EntityLookup } from "./entity-lookup";
import type { EntitySectionStorage } from "./entity-section-storage";

export class LevelEntityGetterAdapter<T extends RuntimeEntityAccess> {
  public constructor(
    private readonly visibleEntities: EntityLookup<T>,
    private readonly sectionStorage: EntitySectionStorage<T>,
  ) {}

  public get(id: number): T | undefined;
  public get(uuid: string): T | undefined;
  public get(idOrUuid: number | string): T | undefined {
    return typeof idOrUuid === "number"
      ? this.visibleEntities.getById(idOrUuid)
      : this.visibleEntities.getByUuid(idOrUuid);
  }

  public getAll(): readonly T[] {
    return this.visibleEntities.getAllEntities();
  }

  public forEachInBox(box: AABB, consumer: (entity: T) => void): void {
    this.sectionStorage.getEntities(box, consumer);
  }

  public getAllInBox(box: AABB): readonly T[] {
    const entities: T[] = [];
    this.forEachInBox(box, (entity) => entities.push(entity));
    return entities;
  }
}
