import { Vec3 } from "../../world/phys/vec3";
import type { CollisionWorld } from "./collision-world";
import type { MovementBody } from "./movement-body";
import type { PlayerMoveCommand } from "./movement-command";
import { simulatePlayerMoveCommands, simulatePlayerMoveCommand } from "./movement-command";
import { MovementCommandBuffer } from "./movement-command-buffer";
import type { MovementPhysicsParams } from "./movement-params";

export type MovementPredictionCauseTag =
  | "replay_backlog"
  | "command_dt_mismatch"
  | "physics_revision_mismatch"
  | "collision_revision_mismatch"
  | "position_error"
  | "velocity_error"
  | "grounded_flip"
  | "jump_state"
  | "missing_collision";

export interface MovementAuthoritativeState {
  readonly body: MovementBody;
  readonly lastProcessedCommandSeq: number;
  readonly physicsRevision: number;
  readonly collisionRevision?: number;
}

export interface MovementReconcileOptions {
  readonly expectedCommandQuantumUs?: number;
  readonly currentPhysicsRevision?: number;
  readonly currentCollisionRevision?: number;
  readonly smallErrorThreshold?: number;
}

export interface MovementBodySnapshot {
  readonly position: Vec3;
  readonly velocity: Vec3;
  readonly onGround: boolean;
  readonly mode: MovementBody["mode"];
  readonly jumpHeld: boolean;
}

export interface MovementReconcileDiagnostics {
  readonly acknowledgedSequence: number;
  readonly replayCount: number;
  readonly replayFirstSequence?: number;
  readonly replayLastSequence?: number;
  readonly correctionPositionError: number;
  readonly correctionVelocityError: number;
  readonly resimPositionError: number;
  readonly resimVelocityError: number;
  readonly predictedBefore: MovementBodySnapshot;
  readonly authoritative: MovementBodySnapshot;
  readonly predictedAfter: MovementBodySnapshot;
  readonly causeTags: readonly MovementPredictionCauseTag[];
}

export interface MovementReconcileResult {
  readonly body: MovementBody;
  readonly diagnostics: MovementReconcileDiagnostics;
  readonly missingCollision: boolean;
}

function bodySnapshot(body: MovementBody): MovementBodySnapshot {
  return {
    position: body.position,
    velocity: body.velocity,
    onGround: body.onGround,
    mode: body.mode,
    jumpHeld: body.jumpHeld,
  };
}

function vecError(left: Vec3, right: Vec3): number {
  return left.subtract(right).length();
}

function addTag(tags: MovementPredictionCauseTag[], tag: MovementPredictionCauseTag): void {
  if (!tags.includes(tag)) {
    tags.push(tag);
  }
}

function collectDiagnosticTags(options: {
  readonly tags: MovementPredictionCauseTag[];
  readonly unacked: readonly PlayerMoveCommand[];
  readonly authoritative: MovementAuthoritativeState;
  readonly predictedBefore: MovementBody;
  readonly predictedAfter: MovementBody;
  readonly correctionPositionError: number;
  readonly correctionVelocityError: number;
  readonly missingCollision: boolean;
  readonly reconcileOptions?: MovementReconcileOptions;
}): void {
  const smallErrorThreshold = options.reconcileOptions?.smallErrorThreshold ?? 1.0e-5;
  if (options.unacked.length > 0) {
    addTag(options.tags, "replay_backlog");
  }
  if (options.correctionPositionError > smallErrorThreshold) {
    addTag(options.tags, "position_error");
  }
  if (options.correctionVelocityError > smallErrorThreshold) {
    addTag(options.tags, "velocity_error");
  }
  if (options.predictedBefore.onGround !== options.authoritative.body.onGround) {
    addTag(options.tags, "grounded_flip");
  }
  if (options.predictedBefore.jumpHeld !== options.authoritative.body.jumpHeld) {
    addTag(options.tags, "jump_state");
  }
  if (options.missingCollision) {
    addTag(options.tags, "missing_collision");
  }

  const expectedQuantum = options.reconcileOptions?.expectedCommandQuantumUs;
  if (expectedQuantum !== undefined && options.unacked.some((command) => command.commandQuantumUs !== expectedQuantum)) {
    addTag(options.tags, "command_dt_mismatch");
  }

  const currentPhysicsRevision = options.reconcileOptions?.currentPhysicsRevision ?? options.authoritative.physicsRevision;
  if (
    currentPhysicsRevision !== options.authoritative.physicsRevision
    || options.unacked.some((command) => command.physicsRevision !== options.authoritative.physicsRevision)
  ) {
    addTag(options.tags, "physics_revision_mismatch");
  }

  const currentCollisionRevision = options.reconcileOptions?.currentCollisionRevision ?? options.authoritative.collisionRevision;
  if (
    currentCollisionRevision !== options.authoritative.collisionRevision
    || options.unacked.some((command) => command.collisionRevision !== options.authoritative.collisionRevision)
  ) {
    addTag(options.tags, "collision_revision_mismatch");
  }
}

export class PlayerMovementPredictor {
  private predicted: MovementBody;
  private readonly buffer: MovementCommandBuffer;
  private lastAck = 0;
  private lastDiagnostics: MovementReconcileDiagnostics | undefined;

  public constructor(initialBody: MovementBody, capacity = 128) {
    this.predicted = initialBody;
    this.buffer = new MovementCommandBuffer(capacity);
  }

  public get predictedBody(): MovementBody {
    return this.predicted;
  }

  public get lastAcknowledgedSequence(): number {
    return this.lastAck;
  }

  public get pendingCommands(): readonly PlayerMoveCommand[] {
    return this.buffer.getUnacknowledged();
  }

  public get lastReconcileDiagnostics(): MovementReconcileDiagnostics | undefined {
    return this.lastDiagnostics;
  }

  public reset(body: MovementBody, acknowledgedSequence = 0): void {
    this.predicted = body;
    this.lastAck = acknowledgedSequence;
    this.buffer.clear();
    this.lastDiagnostics = undefined;
  }

  public applyLocalCommand(command: PlayerMoveCommand, world: CollisionWorld, params: MovementPhysicsParams): MovementBody {
    this.buffer.add(command);
    const result = simulatePlayerMoveCommand(this.predicted, command, world, params);
    this.predicted = result.body;
    return this.predicted;
  }

  public reconcile(
    authoritative: MovementAuthoritativeState,
    world: CollisionWorld,
    params: MovementPhysicsParams,
    options: MovementReconcileOptions = {},
  ): MovementReconcileResult {
    const predictedBefore = this.predicted;
    this.lastAck = Math.max(this.lastAck, authoritative.lastProcessedCommandSeq);
    this.buffer.dropAcknowledged(this.lastAck);
    const unacked = this.buffer.getUnacknowledged();
    const replay = simulatePlayerMoveCommands(authoritative.body, unacked, world, params);
    this.predicted = replay.body;

    const correctionPositionError = vecError(predictedBefore.position, authoritative.body.position);
    const correctionVelocityError = vecError(predictedBefore.velocity, authoritative.body.velocity);
    const resimPositionError = vecError(this.predicted.position, authoritative.body.position);
    const resimVelocityError = vecError(this.predicted.velocity, authoritative.body.velocity);
    const tags: MovementPredictionCauseTag[] = [];
    collectDiagnosticTags({
      tags,
      unacked,
      authoritative,
      predictedBefore,
      predictedAfter: this.predicted,
      correctionPositionError,
      correctionVelocityError,
      missingCollision: replay.missingCollision,
      reconcileOptions: options,
    });

    const diagnostics: MovementReconcileDiagnostics = {
      acknowledgedSequence: this.lastAck,
      replayCount: unacked.length,
      replayFirstSequence: unacked[0]?.sequence,
      replayLastSequence: unacked[unacked.length - 1]?.sequence,
      correctionPositionError,
      correctionVelocityError,
      resimPositionError,
      resimVelocityError,
      predictedBefore: bodySnapshot(predictedBefore),
      authoritative: bodySnapshot(authoritative.body),
      predictedAfter: bodySnapshot(this.predicted),
      causeTags: tags,
    };
    this.lastDiagnostics = diagnostics;

    return {
      body: this.predicted,
      diagnostics,
      missingCollision: replay.missingCollision,
    };
  }
}
