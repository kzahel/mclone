import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import {
  HostMessageClientWorld,
  type ClientWorld,
  type ClientWorldHydrationOptions,
  type ClientWorldHydrationTarget,
  type RenderWorldUpdateSink,
} from "../client/client-world";
import type { WorldClient } from "../protocol/world-client";
import type { WorldHost } from "../protocol/world-host";
import {
  type OpenWorldRequest,
  type PollWorldUpdatesRequest,
  type SetChunkViewRequest,
  type SetPlayerInputRequest,
  type WorldHostMessage,
  type WorldOpenedMessage,
} from "../protocol/world-messages";

export type { RenderWorldChunkUpdateResult, RenderWorldUpdateMessage, RenderWorldUpdateSink } from "../client/client-world";

export interface WorldTransport {
  openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]>;

  setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]>;

  setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]>;

  pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]>;

  supportsChunkViewDeduplication?(): boolean;
}

export interface TransportWorldClientOptions extends ClientWorldHydrationOptions {
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
  private readonly clientWorld: ClientWorldHydrationTarget;
  private lastChunkView: SetChunkViewRequest | undefined;
  private readonly pollUpdateMaxMessages: number | undefined;

  public constructor(
    private readonly transport: WorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options: TransportWorldClientOptions = {},
  ) {
    this.clientWorld = new HostMessageClientWorld(levelFactory, options);
    this.pollUpdateMaxMessages = options.pollUpdateMaxMessages;
  }

  public setRenderWorldUpdateSink(
    chunkUpdateSink: RenderWorldUpdateSink | undefined,
  ): void {
    this.clientWorld.setRenderWorldUpdateSink(chunkUpdateSink);
  }

  public getClientWorld(): ClientWorld {
    return this.clientWorld;
  }

  public getLevel(): ClientChunkCache {
    return this.clientWorld.getLevel();
  }

  public getSessionState(): ReturnType<WorldClient["getSessionState"]> {
    return this.clientWorld.getSessionState();
  }

  public getPlayerState(): ReturnType<WorldClient["getPlayerState"]> {
    return this.clientWorld.getLocalPlayerState();
  }

  public getEntitySnapshots(): ReturnType<WorldClient["getEntitySnapshots"]> {
    return this.clientWorld.getEntitySnapshots();
  }

  public getPerformanceSnapshot(): ReturnType<WorldClient["getPerformanceSnapshot"]> {
    return this.clientWorld.getPerformanceSnapshot();
  }

  public async openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage> {
    const result = await this.clientWorld.hydrateHostMessages(await this.transport.openWorld(request));
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

    const result = await this.clientWorld.hydrateHostMessages(await this.transport.setChunkView(request));
    this.lastChunkView = request;
    return result.chunkChanged;
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<boolean> {
    const result = await this.clientWorld.hydrateHostMessages(await this.transport.setPlayerInput(request));
    return result.messageChanged;
  }

  public async pollUpdates(): Promise<boolean> {
    const result = await this.clientWorld.hydrateHostMessages(await this.transport.pollUpdates({
      type: "poll_world_updates",
      maxMessages: this.pollUpdateMaxMessages,
    }));
    return result.messageChanged;
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
