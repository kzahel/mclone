import { BlockPos } from "../../../core/block-pos";
import { floor } from "../../../util/mth";
import { AABB } from "../../phys/aabb";
import { NULL_ENTITY_IN_LEVEL_CALLBACK, type EntityInLevelCallback } from "./entity-in-level-callback";

export const EntityRemovalReason = {
  KILLED: "killed",
  DISCARDED: "discarded",
  UNLOADED_TO_CHUNK: "unloaded_to_chunk",
  UNLOADED_WITH_PLAYER: "unloaded_with_player",
  CHANGED_DIMENSION: "changed_dimension",
} as const;

export type EntityRemovalReason = typeof EntityRemovalReason[keyof typeof EntityRemovalReason];

const ENTITY_REMOVAL_REASON_DESTROYS: Readonly<Record<EntityRemovalReason, boolean>> = {
  [EntityRemovalReason.KILLED]: true,
  [EntityRemovalReason.DISCARDED]: true,
  [EntityRemovalReason.UNLOADED_TO_CHUNK]: false,
  [EntityRemovalReason.UNLOADED_WITH_PLAYER]: false,
  [EntityRemovalReason.CHANGED_DIMENSION]: false,
};

const ENTITY_REMOVAL_REASON_SAVES: Readonly<Record<EntityRemovalReason, boolean>> = {
  [EntityRemovalReason.KILLED]: false,
  [EntityRemovalReason.DISCARDED]: false,
  [EntityRemovalReason.UNLOADED_TO_CHUNK]: true,
  [EntityRemovalReason.UNLOADED_WITH_PLAYER]: false,
  [EntityRemovalReason.CHANGED_DIMENSION]: false,
};

export interface Vec3Record {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface EntityRotationRecord {
  readonly yaw: number;
  readonly pitch: number;
}

export interface RuntimeEntityAccess {
  readonly id: number;
  readonly uuid: string;
  readonly typeId: string;
  readonly position: Vec3Record;
  readonly rotation: EntityRotationRecord;
  readonly boundingBox: AABB;
  readonly removalReason: EntityRemovalReason | undefined;

  blockPosition(): BlockPos;
  getBoundingBox(): AABB;
  setLevelCallback(callback: EntityInLevelCallback): void;
  setPosition(x: number, y: number, z: number): void;
  setRotation(yaw: number, pitch: number): void;
  setRemoved(reason: EntityRemovalReason): void;
  shouldBeSaved(): boolean;
  isAlwaysTicking(): boolean;
  getSelfAndPassengers(): Iterable<RuntimeEntityAccess>;
  getPassengersAndSelf(): Iterable<RuntimeEntityAccess>;
}

export interface SyntheticRuntimeEntityOptions {
  readonly id: number;
  readonly uuid: string;
  readonly typeId?: string;
  readonly x?: number;
  readonly y?: number;
  readonly z?: number;
  readonly yaw?: number;
  readonly pitch?: number;
  readonly width?: number;
  readonly height?: number;
  readonly alwaysTicking?: boolean;
  readonly save?: boolean;
}

export function entityRemovalReasonShouldDestroy(reason: EntityRemovalReason): boolean {
  return ENTITY_REMOVAL_REASON_DESTROYS[reason];
}

export function entityRemovalReasonShouldSave(reason: EntityRemovalReason): boolean {
  return ENTITY_REMOVAL_REASON_SAVES[reason];
}

export class SyntheticRuntimeEntity implements RuntimeEntityAccess {
  public readonly id: number;
  public readonly uuid: string;
  public readonly typeId: string;
  public position: Vec3Record;
  public rotation: EntityRotationRecord;
  public removalReason: EntityRemovalReason | undefined;
  private readonly width: number;
  private readonly height: number;
  private readonly alwaysTicking: boolean;
  private readonly save: boolean;
  private callback = NULL_ENTITY_IN_LEVEL_CALLBACK;
  private blockPos: BlockPos;
  private box: AABB;

  public constructor(options: SyntheticRuntimeEntityOptions) {
    const x = options.x ?? 0;
    const y = options.y ?? 0;
    const z = options.z ?? 0;
    this.id = options.id;
    this.uuid = options.uuid;
    this.typeId = options.typeId ?? "mclone:test_entity";
    this.position = { x, y, z };
    this.rotation = { yaw: options.yaw ?? 0, pitch: options.pitch ?? 0 };
    this.width = options.width ?? 0.6;
    this.height = options.height ?? 1.8;
    this.alwaysTicking = options.alwaysTicking ?? false;
    this.save = options.save ?? true;
    this.blockPos = new BlockPos(floor(x), floor(y), floor(z));
    this.box = this.createBoundingBox(x, y, z);
  }

  public get boundingBox(): AABB {
    return this.box;
  }

  public blockPosition(): BlockPos {
    return this.blockPos;
  }

  public getBoundingBox(): AABB {
    return this.box;
  }

  public setLevelCallback(callback: EntityInLevelCallback): void {
    this.callback = callback;
  }

  public setPosition(x: number, y: number, z: number): void {
    if (this.position.x === x && this.position.y === y && this.position.z === z) {
      return;
    }

    this.position = { x, y, z };
    this.blockPos = new BlockPos(floor(x), floor(y), floor(z));
    this.box = this.createBoundingBox(x, y, z);
    this.callback.onMove();
  }

  public setRotation(yaw: number, pitch: number): void {
    this.rotation = { yaw, pitch };
  }

  public setRemoved(reason: EntityRemovalReason): void {
    if (this.removalReason === undefined) {
      this.removalReason = reason;
    }

    this.callback.onRemove(reason);
  }

  public shouldBeSaved(): boolean {
    return this.save && (this.removalReason === undefined || entityRemovalReasonShouldSave(this.removalReason));
  }

  public isAlwaysTicking(): boolean {
    return this.alwaysTicking;
  }

  public getSelfAndPassengers(): Iterable<RuntimeEntityAccess> {
    return [this];
  }

  public getPassengersAndSelf(): Iterable<RuntimeEntityAccess> {
    return [this];
  }

  private createBoundingBox(x: number, y: number, z: number): AABB {
    const halfWidth = this.width / 2;
    return new AABB(x - halfWidth, y, z - halfWidth, x + halfWidth, y + this.height, z + halfWidth);
  }
}
