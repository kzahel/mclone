import * as THREE from "three";
import type { ClipLocomotionSpec, Vec3 } from "./dsl";

export interface ReviewFloor {
  root: THREE.Group;
  update(timeSeconds: number): void;
}

export function createReviewFloor(
  bounds: THREE.Box3,
  locomotion: ClipLocomotionSpec | undefined,
  clipDuration: number,
): ReviewFloor {
  const size = bounds.getSize(new THREE.Vector3());
  const cycleDistance = locomotion?.cycleDistance ?? 0;
  const floorSize = Math.max(3, Math.ceil(Math.max(size.x, size.z, cycleDistance * 4) * 3));
  const divisions = Math.max(12, Math.ceil(floorSize / 0.25));
  const cellSize = floorSize / divisions;
  const root = new THREE.Group();
  const basePosition = new THREE.Vector3(0, bounds.min.y - 0.035, 0);
  root.position.copy(basePosition);

  const grid = new THREE.GridHelper(floorSize, divisions, "#7f8b97", "#c3ccd5");
  root.add(grid);

  const direction = horizontalDirection(locomotion?.direction ?? [0, 0, -1]);
  root.add(createStrideLines(floorSize, direction));

  return {
    root,
    update(timeSeconds: number) {
      root.position.copy(basePosition);
      if (!locomotion || clipDuration <= 0 || cycleDistance <= 0) {
        return;
      }
      const distance = (timeSeconds / clipDuration) * cycleDistance;
      const offset = positiveModulo(distance, cellSize);
      root.position.add(direction.clone().multiplyScalar(-offset));
    },
  };
}

export function locomotionSummary(locomotion: ClipLocomotionSpec | undefined): string | undefined {
  if (!locomotion) {
    return undefined;
  }
  const speed = locomotion.speed === undefined ? "" : ` · ${formatNumber(locomotion.speed)} units/s`;
  return `${locomotion.kind} · ${formatNumber(locomotion.cycleDistance)} units/cycle${speed}`;
}

function createStrideLines(floorSize: number, direction: THREE.Vector3): THREE.Object3D {
  const lateral = new THREE.Vector3(-direction.z, 0, direction.x).normalize();
  const half = floorSize / 2;
  const positions: number[] = [];
  for (let step = -half; step <= half; step += 0.5) {
    const center = direction.clone().multiplyScalar(step);
    const left = center.clone().add(lateral.clone().multiplyScalar(-half));
    const right = center.clone().add(lateral.clone().multiplyScalar(half));
    positions.push(left.x, 0.002, left.z, right.x, 0.002, right.z);
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const lines = new THREE.LineSegments(
    geometry,
    new THREE.LineBasicMaterial({
      color: new THREE.Color("#64748b"),
      transparent: true,
      opacity: 0.45,
    }),
  );
  lines.name = "stride_lines";
  return lines;
}

function horizontalDirection(direction: Vec3): THREE.Vector3 {
  const vector = new THREE.Vector3(direction[0], 0, direction[2]);
  if (vector.lengthSq() === 0) {
    return new THREE.Vector3(0, 0, -1);
  }
  return vector.normalize();
}

function positiveModulo(value: number, divisor: number): number {
  return ((value % divisor) + divisor) % divisor;
}

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}
