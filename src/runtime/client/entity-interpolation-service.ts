import type { EntitySnapshot } from "../protocol/world-messages";
import type { ClientWorldEntityView } from "./client-world";

export interface EntityInterpolationServiceOptions {
  readonly interpolationDurationMs?: number;
}

export interface ClientEntityPresentationState {
  readonly entityId: number;
  readonly uuid: string;
  readonly typeId: string;
  readonly category: EntitySnapshot["category"];
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly width: number;
  readonly height: number;
  readonly onGround: boolean;
  readonly age?: number;
  readonly data?: EntitySnapshot["data"];
  readonly interpolatedPosition: EntitySnapshot["position"];
  readonly interpolatedRotation: EntitySnapshot["rotation"];
  readonly interpolationAlpha: number;
  readonly authoritative: EntitySnapshot;
  readonly previousAuthoritative?: EntitySnapshot;
  readonly aiAuthority: "host";
}

interface EntityInterpolationRecord {
  previous: EntitySnapshot;
  current: EntitySnapshot;
  currentUpdateTimeMs: number;
}

const DEFAULT_INTERPOLATION_DURATION_MS = 100;

function clampUnit(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function lerp(left: number, right: number, alpha: number): number {
  return left + ((right - left) * alpha);
}

function lerpDegrees(left: number, right: number, alpha: number): number {
  let delta = (right - left) % 360;
  if (delta > 180) {
    delta -= 360;
  } else if (delta < -180) {
    delta += 360;
  }

  const value = (left + (delta * alpha)) % 360;
  return value < 0 ? value + 360 : value;
}

function shallowDataEqual(left: EntitySnapshot["data"], right: EntitySnapshot["data"]): boolean {
  if (left === right) {
    return true;
  }
  if (left === undefined || right === undefined) {
    return false;
  }

  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);
  if (leftKeys.length !== rightKeys.length) {
    return false;
  }

  return leftKeys.every((key) => left[key] === right[key]);
}

function entitySnapshotEqual(left: EntitySnapshot, right: EntitySnapshot): boolean {
  return left.id === right.id
    && left.uuid === right.uuid
    && left.typeId === right.typeId
    && left.category === right.category
    && left.chunkX === right.chunkX
    && left.chunkZ === right.chunkZ
    && left.position.x === right.position.x
    && left.position.y === right.position.y
    && left.position.z === right.position.z
    && left.rotation.yaw === right.rotation.yaw
    && left.rotation.pitch === right.rotation.pitch
    && left.width === right.width
    && left.height === right.height
    && left.onGround === right.onGround
    && left.age === right.age
    && shallowDataEqual(left.data, right.data);
}

export class ClientEntityInterpolationService {
  private readonly records = new Map<number, EntityInterpolationRecord>();
  private readonly interpolationDurationMs: number;

  public constructor(options: EntityInterpolationServiceOptions = {}) {
    this.interpolationDurationMs = Math.max(0, options.interpolationDurationMs ?? DEFAULT_INTERPOLATION_DURATION_MS);
  }

  public clear(): void {
    this.records.clear();
  }

  public syncClientWorldEntities(entityView: ClientWorldEntityView, updateTimeMs: number): void {
    const snapshots = entityView.getEntitySnapshots();
    const visibleIds = new Set<number>();

    for (const snapshot of snapshots) {
      visibleIds.add(snapshot.id);
      const record = this.records.get(snapshot.id);
      if (record === undefined) {
        this.records.set(snapshot.id, {
          previous: snapshot,
          current: snapshot,
          currentUpdateTimeMs: updateTimeMs,
        });
        continue;
      }

      if (entitySnapshotEqual(record.current, snapshot)) {
        continue;
      }

      this.records.set(snapshot.id, {
        previous: record.current,
        current: snapshot,
        currentUpdateTimeMs: updateTimeMs,
      });
    }

    for (const entityId of this.records.keys()) {
      if (!visibleIds.has(entityId)) {
        this.records.delete(entityId);
      }
    }
  }

  public publishPresentationEntities(sampleTimeMs: number): readonly ClientEntityPresentationState[] {
    return [...this.records.values()]
      .sort((left, right) => left.current.id - right.current.id)
      .map((record) => this.createPresentationState(record, sampleTimeMs));
  }

  private createPresentationState(record: EntityInterpolationRecord, sampleTimeMs: number): ClientEntityPresentationState {
    const alpha = this.computeAlpha(record, sampleTimeMs);
    const previous = record.previous;
    const current = record.current;
    return {
      entityId: current.id,
      uuid: current.uuid,
      typeId: current.typeId,
      category: current.category,
      chunkX: current.chunkX,
      chunkZ: current.chunkZ,
      width: current.width,
      height: current.height,
      onGround: current.onGround,
      age: current.age,
      data: current.data,
      interpolatedPosition: {
        x: lerp(previous.position.x, current.position.x, alpha),
        y: lerp(previous.position.y, current.position.y, alpha),
        z: lerp(previous.position.z, current.position.z, alpha),
      },
      interpolatedRotation: {
        yaw: lerpDegrees(previous.rotation.yaw, current.rotation.yaw, alpha),
        pitch: lerp(previous.rotation.pitch, current.rotation.pitch, alpha),
      },
      interpolationAlpha: alpha,
      authoritative: current,
      previousAuthoritative: previous === current ? undefined : previous,
      aiAuthority: "host",
    };
  }

  private computeAlpha(record: EntityInterpolationRecord, sampleTimeMs: number): number {
    if (record.previous === record.current || this.interpolationDurationMs === 0) {
      return 1;
    }

    const elapsedMs = sampleTimeMs - record.currentUpdateTimeMs;
    return clampUnit(elapsedMs / this.interpolationDurationMs);
  }
}
