import type { RuntimeEntityAccess } from "./entity-access";

export class EntityTickList<T extends RuntimeEntityAccess> {
  private active = new Map<number, T>();
  private passive = new Map<number, T>();
  private iterated: Map<number, T> | undefined;

  public add(entity: T): void {
    this.ensureActiveIsNotIterated();
    this.active.set(entity.id, entity);
  }

  public remove(entity: T): void {
    this.ensureActiveIsNotIterated();
    this.active.delete(entity.id);
  }

  public contains(entity: T): boolean {
    return this.active.has(entity.id);
  }

  public forEach(consumer: (entity: T) => void): void {
    if (this.iterated !== undefined) {
      throw new Error("Only one concurrent iteration supported");
    }

    this.iterated = this.active;
    try {
      for (const entity of this.iterated.values()) {
        consumer(entity);
      }
    } finally {
      this.iterated = undefined;
    }
  }

  public size(): number {
    return this.active.size;
  }

  private ensureActiveIsNotIterated(): void {
    if (this.iterated !== this.active) {
      return;
    }

    this.passive.clear();
    for (const [id, entity] of this.active) {
      this.passive.set(id, entity);
    }

    const previousActive = this.active;
    this.active = this.passive;
    this.passive = previousActive;
  }
}
