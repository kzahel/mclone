import { clamp } from "../../util/mth";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";
import type { CollisionWorld } from "./collision-world";
import type { MovementBody, MovementCollisionFlags, MovementStepResult } from "./movement-body";
import { moveMovementBody } from "./movement-body";
import type { MovementIntent } from "./movement-intent";
import type { MovementPhysicsParams } from "./movement-params";

const EPSILON = 1.0e-7;

interface CollideResult {
  readonly movement: Vec3;
  readonly missingCollision: boolean;
}

function normalizeWish(intent: MovementIntent): { readonly x: number; readonly z: number; readonly active: boolean } {
  const localX = clamp(intent.wishX, -1.0, 1.0);
  const localZ = clamp(intent.wishZ, -1.0, 1.0);
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

function applyHorizontalFriction(velocity: Vec3, frictionPerSecond: number, dt: number): Vec3 {
  const speed = velocity.horizontalDistance();
  if (speed <= EPSILON || frictionPerSecond <= 0.0 || dt <= 0.0) {
    return velocity;
  }

  const nextSpeed = Math.max(0.0, speed - (frictionPerSecond * dt));
  if (nextSpeed === speed) {
    return velocity;
  }

  const scale = nextSpeed / speed;
  return new Vec3(velocity.x * scale, velocity.y, velocity.z * scale);
}

function applyHorizontalAcceleration(velocity: Vec3, wishX: number, wishZ: number, acceleration: number, maxSpeed: number, dt: number): Vec3 {
  const currentAlongWish = (velocity.x * wishX) + (velocity.z * wishZ);
  const addSpeed = maxSpeed - currentAlongWish;
  if (addSpeed <= 0.0) {
    return velocity;
  }

  const accelSpeed = Math.min(acceleration * dt, addSpeed);
  return new Vec3(velocity.x + (wishX * accelSpeed), velocity.y, velocity.z + (wishZ * accelSpeed));
}

function applyDrag(value: number, dragPerSecond: number, dt: number): number {
  if (dragPerSecond <= 0.0 || dt <= 0.0) {
    return value;
  }

  return value * Math.max(0.0, 1.0 - (dragPerSecond * dt));
}

function clipAxis(box: AABB, colliders: readonly AABB[], delta: number, axis: "x" | "y" | "z"): number {
  if (Math.abs(delta) < EPSILON) {
    return 0.0;
  }

  let clipped = delta;
  for (const collider of colliders) {
    if (axis === "x") {
      if (collider.maxY <= box.minY || collider.minY >= box.maxY || collider.maxZ <= box.minZ || collider.minZ >= box.maxZ) {
        continue;
      }
      if (clipped > 0.0 && box.maxX <= collider.minX) {
        clipped = Math.min(clipped, collider.minX - box.maxX);
      } else if (clipped < 0.0 && box.minX >= collider.maxX) {
        clipped = Math.max(clipped, collider.maxX - box.minX);
      }
    } else if (axis === "y") {
      if (collider.maxX <= box.minX || collider.minX >= box.maxX || collider.maxZ <= box.minZ || collider.minZ >= box.maxZ) {
        continue;
      }
      if (clipped > 0.0 && box.maxY <= collider.minY) {
        clipped = Math.min(clipped, collider.minY - box.maxY);
      } else if (clipped < 0.0 && box.minY >= collider.maxY) {
        clipped = Math.max(clipped, collider.maxY - box.minY);
      }
    } else {
      if (collider.maxX <= box.minX || collider.minX >= box.maxX || collider.maxY <= box.minY || collider.minY >= box.maxY) {
        continue;
      }
      if (clipped > 0.0 && box.maxZ <= collider.minZ) {
        clipped = Math.min(clipped, collider.minZ - box.maxZ);
      } else if (clipped < 0.0 && box.minZ >= collider.maxZ) {
        clipped = Math.max(clipped, collider.maxZ - box.minZ);
      }
    }
  }

  return Math.abs(clipped) < EPSILON ? 0.0 : clipped;
}

function collideBoundingBox(movement: Vec3, box: AABB, world: CollisionWorld): CollideResult {
  if (movement.lengthSqr() <= EPSILON * EPSILON) {
    return { movement: Vec3.ZERO, missingCollision: false };
  }

  const query = world.queryBlockCollisions(box.expandTowards(movement).inflate(EPSILON));
  if (query.type === "missing") {
    return { movement: Vec3.ZERO, missingCollision: true };
  }

  let clippedX = movement.x;
  let clippedY = movement.y;
  let clippedZ = movement.z;
  let movedBox = box;

  if (clippedY !== 0.0) {
    clippedY = clipAxis(movedBox, query.boxes, clippedY, "y");
    if (clippedY !== 0.0) {
      movedBox = movedBox.move(0.0, clippedY, 0.0);
    }
  }

  const zFirst = Math.abs(clippedX) < Math.abs(clippedZ);
  if (zFirst && clippedZ !== 0.0) {
    clippedZ = clipAxis(movedBox, query.boxes, clippedZ, "z");
    if (clippedZ !== 0.0) {
      movedBox = movedBox.move(0.0, 0.0, clippedZ);
    }
  }

  if (clippedX !== 0.0) {
    clippedX = clipAxis(movedBox, query.boxes, clippedX, "x");
    if (!zFirst && clippedX !== 0.0) {
      movedBox = movedBox.move(clippedX, 0.0, 0.0);
    }
  }

  if (!zFirst && clippedZ !== 0.0) {
    clippedZ = clipAxis(movedBox, query.boxes, clippedZ, "z");
  }

  return { movement: new Vec3(clippedX, clippedY, clippedZ), missingCollision: false };
}

function collideWithStep(body: MovementBody, movement: Vec3, world: CollisionWorld, maxStepHeight: number): CollideResult {
  const initial = collideBoundingBox(movement, body.bounds, world);
  if (initial.missingCollision || maxStepHeight <= 0.0) {
    return initial;
  }

  const xBlocked = movement.x !== initial.movement.x;
  const yBlocked = movement.y !== initial.movement.y;
  const zBlocked = movement.z !== initial.movement.z;
  const canStep = body.onGround || (yBlocked && movement.y < 0.0);
  if (!canStep || (!xBlocked && !zBlocked)) {
    return initial;
  }

  const directStep = collideBoundingBox(new Vec3(movement.x, maxStepHeight, movement.z), body.bounds, world);
  if (directStep.missingCollision) {
    return directStep;
  }

  const verticalStep = collideBoundingBox(new Vec3(0.0, maxStepHeight, 0.0), body.bounds.expandTowards(movement.x, 0.0, movement.z), world);
  if (verticalStep.missingCollision) {
    return verticalStep;
  }

  let bestStep = directStep.movement;
  if (verticalStep.movement.y < maxStepHeight) {
    const horizontalAfterStep = collideBoundingBox(new Vec3(movement.x, 0.0, movement.z), body.bounds.move(verticalStep.movement), world);
    if (horizontalAfterStep.missingCollision) {
      return horizontalAfterStep;
    }

    const alternateStep = horizontalAfterStep.movement.add(verticalStep.movement);
    if (alternateStep.horizontalDistanceSqr() > bestStep.horizontalDistanceSqr()) {
      bestStep = alternateStep;
    }
  }

  if (bestStep.horizontalDistanceSqr() <= initial.movement.horizontalDistanceSqr()) {
    return initial;
  }

  const down = collideBoundingBox(new Vec3(0.0, -bestStep.y + movement.y, 0.0), body.bounds.move(bestStep), world);
  if (down.missingCollision) {
    return down;
  }

  return { movement: bestStep.add(down.movement), missingCollision: false };
}

function collisionFlags(requested: Vec3, actual: Vec3, missingCollision: boolean): MovementCollisionFlags {
  const x = requested.x !== actual.x;
  const y = requested.y !== actual.y;
  const z = requested.z !== actual.z;
  return {
    x,
    y,
    z,
    horizontal: x || z,
    vertical: y,
    onGround: y && requested.y < 0.0,
    missingCollision,
  };
}

export function simulateMovementStep(
  body: MovementBody,
  intent: MovementIntent,
  world: CollisionWorld,
  fixedDtSeconds: number,
  params: MovementPhysicsParams,
): MovementStepResult {
  if (fixedDtSeconds < 0.0) {
    throw new Error(`fixedDtSeconds must be non-negative, got ${fixedDtSeconds}`);
  }

  if (fixedDtSeconds === 0.0) {
    const collision = collisionFlags(Vec3.ZERO, Vec3.ZERO, false);
    return { body, requestedDisplacement: Vec3.ZERO, actualDisplacement: Vec3.ZERO, collision };
  }

  const wish = normalizeWish(intent);
  const groundedAtStart = body.onGround;
  let velocity = body.velocity;
  if (groundedAtStart) {
    velocity = applyHorizontalFriction(velocity, params.groundFrictionPerSecond, fixedDtSeconds);
  } else {
    velocity = new Vec3(
      applyDrag(velocity.x, params.airDragPerSecond, fixedDtSeconds),
      velocity.y,
      applyDrag(velocity.z, params.airDragPerSecond, fixedDtSeconds),
    );
  }

  if (wish.active) {
    const maxSpeed = (groundedAtStart ? params.maxGroundSpeedBlocksPerSecond : params.maxAirSpeedBlocksPerSecond)
      * (intent.sprint ? params.sprintSpeedMultiplier : 1.0);
    const acceleration = groundedAtStart ? params.groundAccelerationBlocksPerSecondSq : params.airAccelerationBlocksPerSecondSq;
    velocity = applyHorizontalAcceleration(velocity, wish.x, wish.z, acceleration, maxSpeed, fixedDtSeconds);
  }

  let jumped = false;
  if (intent.jump && groundedAtStart && !body.jumpHeld) {
    velocity = new Vec3(velocity.x, params.jumpSpeedBlocksPerSecond, velocity.z);
    jumped = true;
  } else {
    velocity = new Vec3(velocity.x, velocity.y - (params.gravityBlocksPerSecondSq * fixedDtSeconds), velocity.z);
  }

  velocity = new Vec3(velocity.x, applyDrag(velocity.y, params.verticalDragPerSecond, fixedDtSeconds), velocity.z);
  const requestedDisplacement = velocity.scale(fixedDtSeconds);
  const collided = collideWithStep(body, requestedDisplacement, world, params.maxStepHeight);
  const actualDisplacement = collided.movement;
  const baseCollision = collisionFlags(requestedDisplacement, actualDisplacement, collided.missingCollision);
  const steppedOntoGround = groundedAtStart
    && baseCollision.vertical
    && actualDisplacement.y > requestedDisplacement.y
    && requestedDisplacement.y <= 0.0;
  const collision = {
    ...baseCollision,
    onGround: baseCollision.onGround || steppedOntoGround,
  };

  if (collided.missingCollision) {
    return {
      body,
      requestedDisplacement,
      actualDisplacement,
      collision,
    };
  }

  let nextVelocity = velocity;
  if (collision.x) {
    nextVelocity = new Vec3(0.0, nextVelocity.y, nextVelocity.z);
  }
  if (collision.y) {
    nextVelocity = new Vec3(nextVelocity.x, 0.0, nextVelocity.z);
  }
  if (collision.z) {
    nextVelocity = new Vec3(nextVelocity.x, nextVelocity.y, 0.0);
  }

  const onGround = collision.onGround;
  const mode = onGround ? "ground" : "air";
  const jumpHeld = intent.jump || (body.jumpHeld && jumped);
  return {
    body: moveMovementBody(body, actualDisplacement, nextVelocity, onGround, mode, jumpHeld),
    requestedDisplacement,
    actualDisplacement,
    collision,
  };
}
