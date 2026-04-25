import type { RuntimeEntityAccess } from "./entity-access";

export class EntityLookup<T extends RuntimeEntityAccess> {
  private readonly byId = new Map<number, T>();
  private readonly byUuid = new Map<string, T>();

  public add(entity: T): boolean {
    if (this.byUuid.has(entity.uuid)) {
      return false;
    }

    this.byUuid.set(entity.uuid, entity);
    this.byId.set(entity.id, entity);
    return true;
  }

  public remove(entity: T): void {
    this.byUuid.delete(entity.uuid);
    this.byId.delete(entity.id);
  }

  public getById(id: number): T | undefined {
    return this.byId.get(id);
  }

  public getByUuid(uuid: string): T | undefined {
    return this.byUuid.get(uuid);
  }

  public getAllEntities(): readonly T[] {
    return [...this.byId.values()];
  }

  public count(): number {
    return this.byUuid.size;
  }
}
