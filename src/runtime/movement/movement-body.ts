import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";

export type MovementMode = "ground" | "air" | "water" | "flying";

export interface MovementBody {
  readonly position: Vec3;
  readonly velocity: Vec3;
  readonly bounds: AABB;
  readonly onGround: boolean;
  readonly mode: MovementMode;
  readonly jumpHeld: boolean;
}

export interface MovementCollisionFlags {
  readonly x: boolean;
  readonly y: boolean;
  readonly z: boolean;
  readonly horizontal: boolean;
  readonly vertical: boolean;
  readonly onGround: boolean;
  readonly missingCollision: boolean;
}

export interface MovementStepResult {
  readonly body: MovementBody;
  readonly requestedDisplacement: Vec3;
  readonly actualDisplacement: Vec3;
  readonly collision: MovementCollisionFlags;
}

export function createStandingMovementBody(options: {
  readonly position: Vec3;
  readonly velocity?: Vec3;
  readonly width?: number;
  readonly height?: number;
  readonly onGround?: boolean;
  readonly mode?: MovementMode;
  readonly jumpHeld?: boolean;
}): MovementBody {
  const width = options.width ?? 0.6;
  const height = options.height ?? 1.8;
  const halfWidth = width * 0.5;
  const position = options.position;
  return {
    position,
    velocity: options.velocity ?? Vec3.ZERO,
    bounds: new AABB(position.x - halfWidth, position.y, position.z - halfWidth, position.x + halfWidth, position.y + height, position.z + halfWidth),
    onGround: options.onGround ?? false,
    mode: options.mode ?? (options.onGround ? "ground" : "air"),
    jumpHeld: options.jumpHeld ?? false,
  };
}

export function moveMovementBody(body: MovementBody, displacement: Vec3, velocity: Vec3, onGround: boolean, mode: MovementMode, jumpHeld: boolean): MovementBody {
  return {
    position: body.position.add(displacement),
    velocity,
    bounds: body.bounds.move(displacement),
    onGround,
    mode,
    jumpHeld,
  };
}
