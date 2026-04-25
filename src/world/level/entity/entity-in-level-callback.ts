import type { EntityRemovalReason } from "./entity-access";

export interface EntityInLevelCallback {
  onMove(): void;
  onRemove(reason: EntityRemovalReason): void;
}

export const NULL_ENTITY_IN_LEVEL_CALLBACK: EntityInLevelCallback = {
  onMove() {},
  onRemove(_reason: EntityRemovalReason) {},
};
