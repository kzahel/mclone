import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import type { WorldHost } from "../protocol/world-host";
import {
  type ClientSessionState,
  type OpenWorldRequest,
  type SetChunkViewRequest,
  type WorldHostMessage,
  type WorldOpenedMessage,
} from "../protocol/world-messages";

export interface WorldTransport {
  openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]>;

  setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]>;
}

export class LocalWorldTransport implements WorldTransport {
  public constructor(private readonly host: WorldHost) {}

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    // Test/runtime fallback: direct calls exercise the same boundary without a worker hop.
    return this.host.openWorld(request);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    // Test/runtime fallback: direct calls exercise the same boundary without a worker hop.
    return this.host.setChunkView(request);
  }
}

export class TransportWorldClient implements WorldClient {
  private level: ClientChunkCache | undefined;
  private sessionState: ClientSessionState | undefined;

  public constructor(
    private readonly transport: WorldTransport,
    private readonly levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
  ) {}

  public getLevel(): ClientChunkCache {
    if (this.level === undefined) {
      throw new Error("WorldClient.getLevel() called before openWorld()");
    }

    return this.level;
  }

  public getSessionState(): ClientSessionState | undefined {
    return this.sessionState;
  }

  public async openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage> {
    const result = this.applyHostMessages(await this.transport.openWorld(request));
    if (result.worldOpened === undefined) {
      throw new Error("World host did not acknowledge open_world");
    }

    return result.worldOpened;
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<boolean> {
    this.getLevel();
    const result = this.applyHostMessages(await this.transport.setChunkView(request));
    return result.chunkChanged;
  }

  private applyHostMessages(messages: readonly WorldHostMessage[]): {
    readonly worldOpened?: WorldOpenedMessage;
    readonly chunkChanged: boolean;
  } {
    let worldOpened: WorldOpenedMessage | undefined;
    let chunkChanged = false;

    for (const message of messages) {
      switch (message.type) {
        case "world_opened":
          worldOpened = message;
          this.level ??= this.levelFactory(message);
          break;
        case "session_state":
          this.sessionState = message.state;
          break;
        case "chunk_snapshot":
          this.getLevel().applyChunkSnapshot(message.snapshot);
          chunkChanged = true;
          break;
        case "chunk_unload":
          chunkChanged = this.getLevel().applyChunkUnload(message.chunkX, message.chunkZ) || chunkChanged;
          break;
        case "world_error":
          throw new Error(message.message);
      }
    }

    return { worldOpened, chunkChanged };
  }
}

export class LocalWorldClient extends TransportWorldClient {
  public constructor(
    transport: LocalWorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
  ) {
    super(transport, levelFactory);
  }
}
