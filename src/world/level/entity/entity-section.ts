import type { RuntimeEntityAccess } from "./entity-access";
import type { Visibility } from "./visibility";

export class EntitySection<T extends RuntimeEntityAccess> {
  private readonly storage = new Set<T>();

  public constructor(private chunkStatus: Visibility) {}

  public add(entity: T): void {
    this.storage.add(entity);
  }

  public remove(entity: T): boolean {
    return this.storage.delete(entity);
  }

  public getEntities(): Iterable<T> {
    return this.storage.values();
  }

  public forEachEntity(predicate: (entity: T) => boolean, consumer: (entity: T) => void): void {
    for (const entity of this.storage) {
      if (predicate(entity)) {
        consumer(entity);
      }
    }
  }

  public isEmpty(): boolean {
    return this.storage.size === 0;
  }

  public getStatus(): Visibility {
    return this.chunkStatus;
  }

  public updateChunkStatus(status: Visibility): Visibility {
    const previous = this.chunkStatus;
    this.chunkStatus = status;
    return previous;
  }

  public size(): number {
    return this.storage.size;
  }
}
