import { AABB } from "../../world/phys/aabb";
import type { CollisionWorld } from "./collision-world";
import type { MovementBody } from "./movement-body";
import type { MovementIntent } from "./movement-intent";
import type { MovementPhysicsParams } from "./movement-params";

const EPSILON = 1.0e-7;
const AUTO_JUMP_MIN_HEIGHT = 0.5;
const AUTO_JUMP_MAX_HEIGHT = 1.2;
const AUTO_JUMP_LOOKAHEAD_BLOCKS = 0.9;

function getWishDirection(intent: MovementIntent): { readonly x: number; readonly z: number; readonly active: boolean } {
  const localX = Math.max(-1.0, Math.min(1.0, intent.wishX));
  const localZ = Math.max(-1.0, Math.min(1.0, intent.wishZ));
  const length = Math.hypot(localX, localZ);
  if (length <= EPSILON) {
    return { x: 0.0, z: 0.0, active: false };
  }

  const normalizedX = localX / length;
  const normalizedZ = localZ / length;
  const yawRadians = intent.yaw * Math.PI / 180.0;
  const sin = Math.sin(yawRadians);
  const cos = Math.cos(yawRadians);
  return {
    x: (normalizedX * cos) - (normalizedZ * sin),
    z: (normalizedZ * cos) + (normalizedX * sin),
    active: true,
  };
}

function firstAutoJumpObstacle(
  body: MovementBody,
  world: CollisionWorld,
  moveX: number,
  moveZ: number,
): AABB | undefined {
  const probeBounds = body.bounds
    .move(0.0, 0.01, 0.0)
    .expandTowards(moveX, 0.0, moveZ)
    .inflate(EPSILON);
  const query = world.queryBlockCollisions(probeBounds);
  if (query.type === "missing") {
    return undefined;
  }

  let best: AABB | undefined;
  let bestDelta = Number.POSITIVE_INFINITY;
  for (const box of query.boxes) {
    const heightDelta = box.maxY - body.position.y;
    if (heightDelta <= AUTO_JUMP_MIN_HEIGHT || heightDelta > AUTO_JUMP_MAX_HEIGHT) {
      continue;
    }
    if (heightDelta <= body.bounds.maxY - body.position.y && heightDelta < bestDelta) {
      best = box;
      bestDelta = heightDelta;
    }
  }

  return best;
}

function hasLandingClearance(
  body: MovementBody,
  world: CollisionWorld,
  moveX: number,
  moveZ: number,
  heightDelta: number,
): boolean {
  const landingBounds = body.bounds.move(moveX, heightDelta + EPSILON, moveZ);
  const query = world.queryBlockCollisions(landingBounds);
  return query.type === "loaded" && query.boxes.length === 0;
}

export function shouldAutoJump(
  body: MovementBody,
  intent: MovementIntent,
  world: CollisionWorld,
  params: MovementPhysicsParams,
): boolean {
  // Movement protocol: browser auto-jump emits a normal jump edge instead of carrying LocalPlayer.autoJumpTime.
  if (!body.onGround || body.jumpHeld || intent.jump || intent.crouch) {
    return false;
  }

  const wish = getWishDirection(intent);
  if (!wish.active) {
    return false;
  }

  const moveX = wish.x * AUTO_JUMP_LOOKAHEAD_BLOCKS;
  const moveZ = wish.z * AUTO_JUMP_LOOKAHEAD_BLOCKS;
  const obstacle = firstAutoJumpObstacle(body, world, moveX, moveZ);
  if (obstacle === undefined) {
    return false;
  }

  const heightDelta = obstacle.maxY - body.position.y;
  if (heightDelta <= params.maxStepHeight || heightDelta > AUTO_JUMP_MAX_HEIGHT) {
    return false;
  }

  return hasLandingClearance(body, world, moveX, moveZ, heightDelta);
}
