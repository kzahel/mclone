import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";
import { SectionPos } from "../../core/section-pos";
import { AABB } from "../../world/phys/aabb";
import { Vec3 } from "../../world/phys/vec3";
import { blockGetterCollisionWorld, type CollisionWorld } from "../movement/collision-world";
import type {
  MovementAuthoritativeState,
  MovementBody,
} from "../movement";
import type {
  ChunkLightDeltaMessage,
  ChunkSnapshotMessage,
  ChunkUnloadMessage,
  ClientPlayerState,
  ClientSessionState,
  EntitySnapshot,
  WorldHostMessage,
  WorldOpenedMessage,
  WorldPerformanceSnapshot,
  WorldProgressMessage,
} from "../protocol/world-messages";
import type { WorldClient } from "../protocol/world-client";

export type RenderWorldUpdateMessage = ChunkSnapshotMessage | ChunkLightDeltaMessage | ChunkUnloadMessage;

export interface RenderWorldChunkUpdateResult {
  readonly chunkChanged: boolean;
}

export interface RenderWorldUpdateSink {
  ingestUpdates(messages: readonly RenderWorldUpdateMessage[]): Promise<RenderWorldChunkUpdateResult> | RenderWorldChunkUpdateResult;
}

export interface ClientWorldHydrationResult {
  readonly worldOpened?: WorldOpenedMessage;
  readonly chunkChanged: boolean;
  readonly messageChanged: boolean;
}

export interface ClientWorldHydrationOptions {
  readonly chunkUpdateSink?: RenderWorldUpdateSink;
  readonly worldProgressSink?: (message: WorldProgressMessage) => void;
}

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
  getAuthoritativeMovementState(): MovementAuthoritativeState | undefined;
  getEntitySnapshots(): readonly EntitySnapshot[];
  createCollisionWorld(): CollisionWorld;
}

// ClientWorld is a replica hydrated from host updates. It must not generate
// canonical chunks from seed or fill missing authority data through worldgen.
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

export interface ClientWorldHydrationTarget extends ClientWorld {
  getLevel(): ClientChunkCache;
  hydrateHostMessages(messages: readonly WorldHostMessage[]): Promise<ClientWorldHydrationResult>;
  setRenderWorldUpdateSink(chunkUpdateSink: RenderWorldUpdateSink | undefined): void;
}

function boundsChunkRange(bounds: AABB): {
  readonly minChunkX: number;
  readonly maxChunkX: number;
  readonly minChunkZ: number;
  readonly maxChunkZ: number;
} {
  const epsilon = AABB.epsilon();
  return {
    minChunkX: SectionPos.blockToSectionCoord(Math.floor(bounds.minX - epsilon)),
    maxChunkX: SectionPos.blockToSectionCoord(Math.floor(bounds.maxX + epsilon)),
    minChunkZ: SectionPos.blockToSectionCoord(Math.floor(bounds.minZ - epsilon)),
    maxChunkZ: SectionPos.blockToSectionCoord(Math.floor(bounds.maxZ + epsilon)),
  };
}

function clientChunkCacheCollisionWorld(level: ClientChunkCache): CollisionWorld {
  const loadedWorld = blockGetterCollisionWorld(level);
  return {
    queryBlockCollisions(bounds) {
      const range = boundsChunkRange(bounds);
      for (let chunkZ = range.minChunkZ; chunkZ <= range.maxChunkZ; chunkZ++) {
        for (let chunkX = range.minChunkX; chunkX <= range.maxChunkX; chunkX++) {
          if (level.getChunkSnapshot(chunkX, chunkZ) === undefined) {
            return {
              type: "missing",
              reason: `missing_chunk:${chunkX.toString()},${chunkZ.toString()}`,
            };
          }
        }
      }

      return loadedWorld.queryBlockCollisions(bounds);
    },
  };
}

function movementBodyFromClientSnapshot(
  snapshot: NonNullable<ClientPlayerState["movementBody"]>,
): MovementBody {
  return {
    position: new Vec3(snapshot.position.x, snapshot.position.y, snapshot.position.z),
    velocity: new Vec3(snapshot.velocity.x, snapshot.velocity.y, snapshot.velocity.z),
    bounds: new AABB(
      snapshot.bounds.minX,
      snapshot.bounds.minY,
      snapshot.bounds.minZ,
      snapshot.bounds.maxX,
      snapshot.bounds.maxY,
      snapshot.bounds.maxZ,
    ),
    onGround: snapshot.onGround,
    mode: snapshot.mode,
    jumpHeld: snapshot.jumpHeld,
  };
}

function authoritativeMovementStateFromPlayerState(
  playerState: ClientPlayerState | undefined,
): MovementAuthoritativeState | undefined {
  const movementBody = playerState?.movementBody;
  if (movementBody === undefined) {
    return undefined;
  }

  return {
    body: movementBodyFromClientSnapshot(movementBody),
    lastProcessedCommandSeq: movementBody.lastProcessedCommandSequence,
    physicsRevision: movementBody.physicsRevision,
    collisionRevision: movementBody.collisionRevision,
  };
}

export class HostMessageClientWorld implements ClientWorldHydrationTarget {
  private level: ClientChunkCache | undefined;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;
  private readonly entitySnapshots = new Map<number, EntitySnapshot>();
  private performanceSnapshot: WorldPerformanceSnapshot | undefined;
  private chunkUpdateSink: RenderWorldUpdateSink | undefined;
  private readonly worldProgressSink: ((message: WorldProgressMessage) => void) | undefined;

  public constructor(
    private readonly levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options: ClientWorldHydrationOptions = {},
  ) {
    this.chunkUpdateSink = options.chunkUpdateSink;
    this.worldProgressSink = options.worldProgressSink;
  }

  public setRenderWorldUpdateSink(chunkUpdateSink: RenderWorldUpdateSink | undefined): void {
    this.chunkUpdateSink = chunkUpdateSink;
  }

  public getLevel(): ClientChunkCache {
    if (this.level === undefined) {
      throw new Error("ClientWorld.getLevel() called before world_opened");
    }

    return this.level;
  }

  public getSessionState(): ClientSessionState | undefined {
    return this.sessionState;
  }

  public getLocalPlayerState(): ClientPlayerState | undefined {
    return this.playerState;
  }

  public getEntitySnapshots(): readonly EntitySnapshot[] {
    return [...this.entitySnapshots.values()];
  }

  public getPerformanceSnapshot(): WorldPerformanceSnapshot | undefined {
    return this.performanceSnapshot;
  }

  public getChunkSnapshot(chunkX: number, chunkZ: number): ChunkSnapshot | undefined {
    return this.getLevel().getChunkSnapshot(chunkX, chunkZ);
  }

  public getRevisionFacts(): ClientWorldRevisionFacts {
    return {
      sessionRevision: this.sessionState?.revision,
      localPlayerRevision: this.playerState?.revision,
      movementPhysicsRevision: this.playerState?.movementBody?.physicsRevision,
      collisionRevision: this.playerState?.movementBody?.collisionRevision,
    };
  }

  public getRenderView(): ClientWorldRenderView {
    return {
      getRenderLevel: () => this.getLevel(),
    };
  }

  public getPredictionView(): ClientWorldPredictionView {
    return {
      movementPhysicsRevision: this.playerState?.movementBody?.physicsRevision,
      collisionRevision: this.playerState?.movementBody?.collisionRevision,
      getAuthoritativeMovementState: () => authoritativeMovementStateFromPlayerState(this.playerState),
      getEntitySnapshots: () => this.getEntitySnapshots(),
      createCollisionWorld: () => clientChunkCacheCollisionWorld(this.getLevel()),
    };
  }

  public async hydrateHostMessages(messages: readonly WorldHostMessage[]): Promise<ClientWorldHydrationResult> {
    let worldOpened: WorldOpenedMessage | undefined;
    let chunkChanged = false;
    let messageChanged = false;
    const pendingChunkUpdates: RenderWorldUpdateMessage[] = [];

    const flushChunkUpdates = async (): Promise<void> => {
      if (pendingChunkUpdates.length <= 0) {
        return;
      }

      const updates = [...pendingChunkUpdates];
      pendingChunkUpdates.length = 0;
      if (this.chunkUpdateSink !== undefined) {
        const result = await this.chunkUpdateSink.ingestUpdates(updates);
        chunkChanged = result.chunkChanged || chunkChanged;
      }
    };

    for (const message of messages) {
      switch (message.type) {
        case "world_opened":
          worldOpened = message;
          chunkChanged = this.applyWorldOpened(message, pendingChunkUpdates) || chunkChanged;
          break;
        case "session_state":
          this.sessionState = message.state;
          messageChanged = true;
          break;
        case "player_state":
          this.playerState = message.state;
          messageChanged = true;
          break;
        case "entity_snapshot":
          this.entitySnapshots.set(message.entity.id, message.entity);
          messageChanged = true;
          break;
        case "chunk_snapshot":
          pendingChunkUpdates.push(message);
          this.getLevel().applyPackedChunkSnapshot(message.snapshot);
          chunkChanged = true;
          messageChanged = true;
          break;
        case "chunk_light_delta":
          pendingChunkUpdates.push(message);
          chunkChanged = this.getLevel().applyChunkLightDelta(message) || chunkChanged;
          messageChanged = true;
          break;
        case "chunk_unload":
          for (const [entityId, entity] of this.entitySnapshots) {
            if (entity.chunkX === message.chunkX && entity.chunkZ === message.chunkZ) {
              this.entitySnapshots.delete(entityId);
            }
          }
          pendingChunkUpdates.push(message);
          chunkChanged = this.getLevel().applyChunkUnload(message.chunkX, message.chunkZ) || chunkChanged;
          messageChanged = true;
          break;
        case "world_progress":
          this.worldProgressSink?.(message);
          messageChanged = true;
          break;
        case "world_perf":
          this.performanceSnapshot = message.performance;
          break;
        case "world_error":
          await flushChunkUpdates();
          throw new Error(message.message);
      }
    }

    await flushChunkUpdates();
    return { worldOpened, chunkChanged, messageChanged };
  }

  private applyWorldOpened(message: WorldOpenedMessage, pendingChunkUpdates: RenderWorldUpdateMessage[]): boolean {
    const loadedChunks = this.level?.getLoadedChunks() ?? [];
    for (const chunk of loadedChunks) {
      pendingChunkUpdates.push({
        type: "chunk_unload",
        chunkX: chunk.chunkX,
        chunkZ: chunk.chunkZ,
      });
    }

    this.level = this.levelFactory(message);
    this.sessionState = undefined;
    this.playerState = undefined;
    this.entitySnapshots.clear();
    this.performanceSnapshot = undefined;
    return loadedChunks.length > 0;
  }
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
      getAuthoritativeMovementState: () => authoritativeMovementStateFromPlayerState(this.client.getPlayerState()),
      getEntitySnapshots: () => this.client.getEntitySnapshots(),
      createCollisionWorld: () => clientChunkCacheCollisionWorld(this.client.getLevel()),
    };
  }
}
