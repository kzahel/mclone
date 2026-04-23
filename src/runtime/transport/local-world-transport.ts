import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import type { WorldHost } from "../protocol/world-host";
import {
  type OpenWorldRequest,
  type SetChunkViewRequest,
  type WorldHostMessage,
  type WorldOpenedMessage,
} from "../protocol/world-messages";

export class LocalWorldTransport {
  public constructor(private readonly host: WorldHost) {}

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    // Browser runtime: in-process transport stands in for the worker/socket boundary until R1.
    return this.host.openWorld(request);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    // Browser runtime: in-process transport stands in for the worker/socket boundary until R1.
    return this.host.setChunkView(request);
  }
}

export class LocalWorldClient implements WorldClient {
  private level: ClientChunkCache | undefined;

  public constructor(
    private readonly transport: LocalWorldTransport,
    private readonly levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
  ) {}

  public getLevel(): ClientChunkCache {
    if (this.level === undefined) {
      throw new Error("WorldClient.getLevel() called before openWorld()");
    }

    return this.level;
  }

  public async openWorld(request: OpenWorldRequest = { type: "open_world" }): Promise<WorldOpenedMessage> {
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
