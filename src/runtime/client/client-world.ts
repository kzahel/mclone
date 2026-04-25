import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";
import { blockGetterCollisionWorld, type CollisionWorld } from "../movement/collision-world";
import type {
  ClientPlayerState,
  ClientSessionState,
  EntitySnapshot,
  WorldPerformanceSnapshot,
} from "../protocol/world-messages";
import type { WorldClient } from "../protocol/world-client";

export interface ClientWorldRevisionFacts {
  readonly sessionRevision?: number;
  readonly localPlayerRevision?: number;
  readonly movementPhysicsRevision?: number;
  readonly collisionRevision?: number;
}

export interface ClientWorldRenderView {
  getRenderLevel(): ClientChunkCache;
}

export interface ClientWorldPredictionView {
  readonly movementPhysicsRevision?: number;
  readonly collisionRevision?: number;
  createCollisionWorld(): CollisionWorld;
}

export interface ClientWorld {
  getSessionState(): ClientSessionState | undefined;
  getLocalPlayerState(): ClientPlayerState | undefined;
  getEntitySnapshots(): readonly EntitySnapshot[];
  getPerformanceSnapshot(): WorldPerformanceSnapshot | undefined;
  getChunkSnapshot(chunkX: number, chunkZ: number): ChunkSnapshot | undefined;
  getRevisionFacts(): ClientWorldRevisionFacts;
  getRenderView(): ClientWorldRenderView;
  getPredictionView(): ClientWorldPredictionView;
}

export class WorldClientBackedClientWorld implements ClientWorld {
  public constructor(private readonly client: WorldClient) {}

  public getSessionState(): ClientSessionState | undefined {
    return this.client.getSessionState();
  }

  public getLocalPlayerState(): ClientPlayerState | undefined {
    return this.client.getPlayerState();
  }

  public getEntitySnapshots(): readonly EntitySnapshot[] {
    return this.client.getEntitySnapshots();
  }

  public getPerformanceSnapshot(): WorldPerformanceSnapshot | undefined {
    return this.client.getPerformanceSnapshot();
  }

  public getChunkSnapshot(chunkX: number, chunkZ: number): ChunkSnapshot | undefined {
    return this.client.getLevel().getChunkSnapshot(chunkX, chunkZ);
  }

  public getRevisionFacts(): ClientWorldRevisionFacts {
    const session = this.client.getSessionState();
    const player = this.client.getPlayerState();
    return {
      sessionRevision: session?.revision,
      localPlayerRevision: player?.revision,
      movementPhysicsRevision: player?.movementBody?.physicsRevision,
      collisionRevision: player?.movementBody?.collisionRevision,
    };
  }

  public getRenderView(): ClientWorldRenderView {
    return {
      getRenderLevel: () => this.client.getLevel(),
    };
  }

  public getPredictionView(): ClientWorldPredictionView {
    return {
      movementPhysicsRevision: this.client.getPlayerState()?.movementBody?.physicsRevision,
      collisionRevision: this.client.getPlayerState()?.movementBody?.collisionRevision,
      createCollisionWorld: () => blockGetterCollisionWorld(this.client.getLevel()),
    };
  }
}
