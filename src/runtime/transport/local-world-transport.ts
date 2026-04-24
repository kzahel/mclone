import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import type { WorldHost } from "../protocol/world-host";
import {
  type ClientPlayerState,
  type ClientSessionState,
  type ChunkSnapshotMessage,
  type ChunkUnloadMessage,
  type OpenWorldRequest,
  type PollWorldUpdatesRequest,
  type SetChunkViewRequest,
  type SetPlayerInputRequest,
  type WorldHostMessage,
  type WorldOpenedMessage,
} from "../protocol/world-messages";

export interface WorldTransport {
  openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]>;

  setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]>;

  setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]>;

  pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]>;

  supportsChunkViewDeduplication?(): boolean;
}

export type RenderWorldUpdateMessage = ChunkSnapshotMessage | ChunkUnloadMessage;

export interface RenderWorldChunkUpdateResult {
  readonly chunkChanged: boolean;
}

export interface RenderWorldUpdateSink {
  ingestUpdates(messages: readonly RenderWorldUpdateMessage[]): Promise<RenderWorldChunkUpdateResult> | RenderWorldChunkUpdateResult;
}

export interface TransportWorldClientOptions {
  readonly chunkUpdateSink?: RenderWorldUpdateSink;
  readonly mirrorChunkUpdatesToLevel?: boolean;
  readonly pollUpdateMaxMessages?: number;
}

export class LocalWorldTransport implements WorldTransport {
  public constructor(private readonly host: WorldHost) {}

  public supportsChunkViewDeduplication(): boolean {
    return true;
  }

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    // Test/runtime fallback: direct calls exercise the same boundary without a worker hop.
    return this.host.openWorld(request);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    // Test/runtime fallback: direct calls exercise the same boundary without a worker hop.
    return this.host.setChunkView(request);
  }

  public setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return this.host.setPlayerInput(request);
  }

  public pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return this.host.pollUpdates(request);
  }
}

export class TransportWorldClient implements WorldClient {
  private level: ClientChunkCache | undefined;
  private sessionState: ClientSessionState | undefined;
  private playerState: ClientPlayerState | undefined;
  private lastChunkView: SetChunkViewRequest | undefined;
  private chunkUpdateSink: RenderWorldUpdateSink | undefined;
  private mirrorChunkUpdatesToLevel: boolean;
  private readonly pollUpdateMaxMessages: number | undefined;

  public constructor(
    private readonly transport: WorldTransport,
    private readonly levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options: TransportWorldClientOptions = {},
  ) {
    this.chunkUpdateSink = options.chunkUpdateSink;
    this.mirrorChunkUpdatesToLevel = options.mirrorChunkUpdatesToLevel ?? options.chunkUpdateSink === undefined;
    this.pollUpdateMaxMessages = options.pollUpdateMaxMessages;
  }

  public setRenderWorldUpdateSink(
    chunkUpdateSink: RenderWorldUpdateSink | undefined,
    options: { readonly mirrorChunkUpdatesToLevel?: boolean } = {},
  ): void {
    this.chunkUpdateSink = chunkUpdateSink;
    this.mirrorChunkUpdatesToLevel = options.mirrorChunkUpdatesToLevel ?? chunkUpdateSink === undefined;
  }

  public getLevel(): ClientChunkCache {
    if (this.level === undefined) {
      throw new Error("WorldClient.getLevel() called before openWorld()");
    }

    return this.level;
  }

  public getSessionState(): ClientSessionState | undefined {
    return this.sessionState;
  }

  public getPlayerState(): ClientPlayerState | undefined {
    return this.playerState;
  }

  public async openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage> {
    const result = await this.applyHostMessages(await this.transport.openWorld(request));
    if (result.worldOpened === undefined) {
      throw new Error("World host did not acknowledge open_world");
    }

    this.lastChunkView = undefined;
    return result.worldOpened;
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<boolean> {
    this.getLevel();
    if (
      this.transport.supportsChunkViewDeduplication?.() !== false
      &&
      this.lastChunkView !== undefined
      && this.lastChunkView.centerChunkX === request.centerChunkX
      && this.lastChunkView.centerChunkZ === request.centerChunkZ
      && this.lastChunkView.radius === request.radius
    ) {
      return false;
    }

    const result = await this.applyHostMessages(await this.transport.setChunkView(request));
    this.lastChunkView = request;
    return result.chunkChanged;
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<boolean> {
    const result = await this.applyHostMessages(await this.transport.setPlayerInput(request));
    return result.messageChanged;
  }

  public async pollUpdates(): Promise<boolean> {
    const result = await this.applyHostMessages(await this.transport.pollUpdates({
      type: "poll_world_updates",
      maxMessages: this.pollUpdateMaxMessages,
    }));
    return result.messageChanged;
  }

  private async applyHostMessages(messages: readonly WorldHostMessage[]): Promise<{
    readonly worldOpened?: WorldOpenedMessage;
    readonly chunkChanged: boolean;
    readonly messageChanged: boolean;
  }> {
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
          this.level ??= this.levelFactory(message);
          break;
        case "session_state":
          this.sessionState = message.state;
          messageChanged = true;
          break;
        case "player_state":
          this.playerState = message.state;
          messageChanged = true;
          break;
        case "chunk_snapshot":
          if (this.chunkUpdateSink !== undefined) {
            pendingChunkUpdates.push(message);
          }
          if (this.chunkUpdateSink === undefined || this.mirrorChunkUpdatesToLevel) {
            this.getLevel().applyPackedChunkSnapshot(message.snapshot);
            chunkChanged = true;
          }
          messageChanged = true;
          break;
        case "chunk_unload":
          if (this.chunkUpdateSink !== undefined) {
            pendingChunkUpdates.push(message);
          }
          if (this.chunkUpdateSink === undefined || this.mirrorChunkUpdatesToLevel) {
            chunkChanged = this.getLevel().applyChunkUnload(message.chunkX, message.chunkZ) || chunkChanged;
          }
          messageChanged = true;
          break;
        case "world_error":
          await flushChunkUpdates();
          throw new Error(message.message);
      }
    }

    await flushChunkUpdates();
    return { worldOpened, chunkChanged, messageChanged };
  }
}

export class LocalWorldClient extends TransportWorldClient {
  public constructor(
    transport: LocalWorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options?: TransportWorldClientOptions,
  ) {
    super(transport, levelFactory, options);
  }
}
