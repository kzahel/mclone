import * as THREE from "three";
import type { ClipSpec, FigureAsset, PartSpec, Vec3 } from "./dsl";

export interface FigurePose {
  clipName?: string;
  time: number;
}

/**
 * Reconstructs the part-content matrices used by the preview scene without
 * constructing renderer objects. Children inherit the parent content matrix,
 * including its pivot compensation and sampled clip transform.
 */
export function evaluateFigurePose(
  asset: FigureAsset,
  pose: FigurePose = { time: 0 },
): Map<string, THREE.Matrix4> {
  const clip = pose.clipName === undefined ? undefined : asset.clips[pose.clipName];
  if (pose.clipName !== undefined && clip === undefined) {
    throw new Error(`Figure '${asset.name}' has no clip '${pose.clipName}'`);
  }
  const localTime = clip === undefined ? 0 : clipTime(clip, pose.time);
  const keysByPart = clip === undefined ? new Map<string, ClipSpec["keys"]>() : prepareKeys(clip);
  const parts = new Map(asset.parts.map((part) => [part.name, part] as const));
  const matrices = new Map<string, THREE.Matrix4>();
  const visiting = new Set<string>();

  const resolve = (part: PartSpec): THREE.Matrix4 => {
    const cached = matrices.get(part.name);
    if (cached) {
      return cached;
    }
    if (visiting.has(part.name)) {
      return new THREE.Matrix4();
    }
    visiting.add(part.name);

    const parent = part.parent === undefined ? undefined : parts.get(part.parent);
    const parentMatrix = parent === undefined ? new THREE.Matrix4() : resolve(parent);
    const pivot = vector(part.joint?.pivot ?? part.pivot);
    const keys = keysByPart.get(part.name) ?? [];
    const sampledPosition = sampleChannel(keys, localTime, (key) => key[2].at);
    const position = vector(part.at).add(pivot);
    if (sampledPosition) {
      position.add(vector(sampledPosition));
    }

    const baseRotation = euler(part.rot);
    const rotationSpan = channelSpan(keys, localTime, (key) => key[2].rot);
    const rotation = rotationSpan === undefined
      ? new THREE.Quaternion().setFromEuler(baseRotation)
      : new THREE.Quaternion().slerpQuaternions(
        additiveRotation(baseRotation, rotationSpan.left),
        additiveRotation(baseRotation, rotationSpan.right),
        rotationSpan.alpha,
      ).normalize();
    const sampledScale = sampleChannel(keys, localTime, (key) => key[2].scale);
    const scale = vector(sampledScale ?? [1, 1, 1]);
    const localMatrix = new THREE.Matrix4()
      .compose(position, rotation, scale)
      .multiply(new THREE.Matrix4().makeTranslation(-pivot.x, -pivot.y, -pivot.z));
    const matrix = parentMatrix.clone().multiply(localMatrix);
    matrices.set(part.name, matrix);
    visiting.delete(part.name);
    return matrix;
  };

  for (const part of asset.parts) {
    resolve(part);
  }
  return matrices;
}

/** Rest, authored keys, key midpoints, and a bounded uniform clip cadence. */
export function figureAnalysisPoses(asset: FigureAsset): FigurePose[] {
  const poses: FigurePose[] = [{ time: 0 }];
  for (const [clipName, clip] of Object.entries(asset.clips)) {
    const duration = clipDuration(clip);
    const times = new Set<number>([0, duration]);
    for (const [, time] of clip.keys) {
      times.add(THREE.MathUtils.clamp(time, 0, duration));
    }
    const authored = [...times].sort((left, right) => left - right);
    for (let index = 1; index < authored.length; index += 1) {
      times.add((authored[index - 1]! + authored[index]!) / 2);
    }
    const uniformIntervals = Math.min(
      24,
      Math.max(2, Math.ceil(duration * (clip.fps ?? 12))),
    );
    for (let index = 0; index <= uniformIntervals; index += 1) {
      times.add(duration * index / uniformIntervals);
    }
    for (const time of [...times].sort((left, right) => left - right)) {
      poses.push({ clipName, time });
    }
  }
  return poses;
}

interface ChannelSpan {
  left: Vec3;
  right: Vec3;
  alpha: number;
}

function clipTime(clip: ClipSpec, time: number): number {
  if (!Number.isFinite(time)) {
    throw new Error("Animation analysis time must be finite");
  }
  const duration = clipDuration(clip);
  return clip.loop && duration > 0
    ? ((time % duration) + duration) % duration
    : THREE.MathUtils.clamp(time, 0, duration);
}

function clipDuration(clip: ClipSpec): number {
  return Math.max(0, ...clip.keys.map(([, time]) => time));
}

function prepareKeys(clip: ClipSpec): Map<string, ClipSpec["keys"]> {
  const keysByPart = new Map<string, ClipSpec["keys"]>();
  for (const key of clip.keys) {
    const keys = keysByPart.get(key[0]) ?? [];
    keys.push(key);
    keysByPart.set(key[0], keys);
  }
  for (const keys of keysByPart.values()) {
    keys.sort((left, right) => left[1] - right[1]);
  }
  return keysByPart;
}

function sampleChannel(
  keys: ClipSpec["keys"],
  time: number,
  channel: (key: ClipSpec["keys"][number]) => Vec3 | undefined,
): Vec3 | undefined {
  const span = channelSpan(keys, time, channel);
  if (!span) {
    return undefined;
  }
  return [
    THREE.MathUtils.lerp(span.left[0], span.right[0], span.alpha),
    THREE.MathUtils.lerp(span.left[1], span.right[1], span.alpha),
    THREE.MathUtils.lerp(span.left[2], span.right[2], span.alpha),
  ];
}

function channelSpan(
  keys: ClipSpec["keys"],
  time: number,
  channel: (key: ClipSpec["keys"][number]) => Vec3 | undefined,
): ChannelSpan | undefined {
  const keyed = keys.flatMap((key) => {
    const value = channel(key);
    return value ? [{ time: key[1], value }] : [];
  });
  const first = keyed[0];
  if (!first) {
    return undefined;
  }
  let left = first;
  let right = first;
  for (const current of keyed.slice(1)) {
    if (time < current.time) {
      right = current;
      break;
    }
    left = current;
    right = current;
  }
  return {
    left: left.value,
    right: right.value,
    alpha: right.time === left.time
      ? 0
      : THREE.MathUtils.clamp((time - left.time) / (right.time - left.time), 0, 1),
  };
}

function additiveRotation(base: THREE.Euler, delta: Vec3): THREE.Quaternion {
  return new THREE.Quaternion().setFromEuler(new THREE.Euler(
    base.x + THREE.MathUtils.degToRad(delta[0]),
    base.y + THREE.MathUtils.degToRad(delta[1]),
    base.z + THREE.MathUtils.degToRad(delta[2]),
    "XYZ",
  ));
}

function vector(value: Vec3 | undefined): THREE.Vector3 {
  return new THREE.Vector3(...(value ?? [0, 0, 0]));
}

function euler(value: Vec3 | undefined): THREE.Euler {
  const [x, y, z] = value ?? [0, 0, 0];
  return new THREE.Euler(
    THREE.MathUtils.degToRad(x),
    THREE.MathUtils.degToRad(y),
    THREE.MathUtils.degToRad(z),
    "XYZ",
  );
}
