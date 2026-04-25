import type { MovementBody } from "../movement/movement-body";
import type { PlayerMoveCommand } from "../movement/movement-command";
import {
  PlayerMovementPredictor,
  type MovementAuthoritativeState,
  type MovementReconcileDiagnostics,
  type MovementReconcileOptions,
  type MovementReconcileResult,
} from "../movement/movement-predictor";
import type { MovementPhysicsParams } from "../movement/movement-params";
import type { ClientWorldPredictionView } from "./client-world";

type MutableMovementReconcileOptions = {
  -readonly [Key in keyof MovementReconcileOptions]: MovementReconcileOptions[Key];
};

export interface PredictionCommandReplayRequest {
  readonly command: PlayerMoveCommand;
  readonly clientWorld: ClientWorldPredictionView;
  readonly physicsParams: MovementPhysicsParams;
}

export interface PredictionReconcileRequest {
  readonly authoritative: MovementAuthoritativeState;
  readonly clientWorld: ClientWorldPredictionView;
  readonly physicsParams: MovementPhysicsParams;
  readonly options?: MovementReconcileOptions;
}

export interface PredictionClientWorldReconcileRequest {
  readonly clientWorld: ClientWorldPredictionView;
  readonly physicsParams: MovementPhysicsParams;
  readonly options?: MovementReconcileOptions;
}

export interface PredictionService {
  getPredictedBody(): MovementBody;
  getLastAcknowledgedSequence(): number;
  getPendingCommands(): readonly PlayerMoveCommand[];
  getLastDiagnostics(): MovementReconcileDiagnostics | undefined;
  resetFromAuthoritativeBody(body: MovementBody, acknowledgedSequence?: number): void;
  advanceCommandReplay(request: PredictionCommandReplayRequest): MovementBody;
  reconcileAuthoritativeSnapshot(request: PredictionReconcileRequest): MovementReconcileResult;
  reconcileClientWorldSnapshot(request: PredictionClientWorldReconcileRequest): MovementReconcileResult | undefined;
}

function mergeClientWorldReconcileOptions(
  clientWorld: ClientWorldPredictionView,
  options: MovementReconcileOptions | undefined,
): MovementReconcileOptions {
  const merged: MutableMovementReconcileOptions = { ...(options ?? {}) };
  if (merged.currentPhysicsRevision === undefined && clientWorld.movementPhysicsRevision !== undefined) {
    merged.currentPhysicsRevision = clientWorld.movementPhysicsRevision;
  }
  if (merged.currentCollisionRevision === undefined && clientWorld.collisionRevision !== undefined) {
    merged.currentCollisionRevision = clientWorld.collisionRevision;
  }

  return merged;
}

export class PlayerMovementPredictionService implements PredictionService {
  public constructor(private readonly predictor: PlayerMovementPredictor) {}

  public getPredictedBody(): MovementBody {
    return this.predictor.predictedBody;
  }

  public getLastAcknowledgedSequence(): number {
    return this.predictor.lastAcknowledgedSequence;
  }

  public getPendingCommands(): readonly PlayerMoveCommand[] {
    return this.predictor.pendingCommands;
  }

  public getLastDiagnostics(): MovementReconcileDiagnostics | undefined {
    return this.predictor.lastReconcileDiagnostics;
  }

  public resetFromAuthoritativeBody(body: MovementBody, acknowledgedSequence = 0): void {
    this.predictor.reset(body, acknowledgedSequence);
  }

  public advanceCommandReplay(request: PredictionCommandReplayRequest): MovementBody {
    return this.predictor.applyLocalCommand(
      request.command,
      request.clientWorld.createCollisionWorld(),
      request.physicsParams,
    );
  }

  public reconcileAuthoritativeSnapshot(request: PredictionReconcileRequest): MovementReconcileResult {
    return this.predictor.reconcile(
      request.authoritative,
      request.clientWorld.createCollisionWorld(),
      request.physicsParams,
      mergeClientWorldReconcileOptions(request.clientWorld, request.options),
    );
  }

  public reconcileClientWorldSnapshot(request: PredictionClientWorldReconcileRequest): MovementReconcileResult | undefined {
    const authoritative = request.clientWorld.getAuthoritativeMovementState();
    if (authoritative === undefined) {
      return undefined;
    }

    return this.reconcileAuthoritativeSnapshot({
      ...request,
      authoritative,
    });
  }
}
