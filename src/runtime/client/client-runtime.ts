import type {
  ClientPlayerState,
  ClientSessionState,
  EntitySnapshot,
  OpenWorldRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldOpenedMessage,
  WorldPerformanceSnapshot,
} from "../protocol/world-messages";
import type { WorldClient } from "../protocol/world-client";
import { type ClientWorld, type RenderWorldUpdateSink, WorldClientBackedClientWorld } from "./client-world";

export interface ClientPresentationState {
  readonly sessionState?: ClientSessionState;
  readonly localPlayerState?: ClientPlayerState;
  readonly entities: readonly EntitySnapshot[];
  readonly performance?: WorldPerformanceSnapshot;
}

export interface ClientRuntime {
  openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage>;
  setChunkInterest(request: SetChunkViewRequest): Promise<boolean>;
  sendPlayerCommand(request: SetPlayerInputRequest): Promise<boolean>;
  drainTransportUpdates(): Promise<boolean>;
  setRenderWorldUpdateSink(sink: RenderWorldUpdateSink | undefined): void;
  getClientWorld(): ClientWorld;
  publishPresentationState(): ClientPresentationState;
}

interface RenderWorldUpdateSinkTarget {
  setRenderWorldUpdateSink(sink: RenderWorldUpdateSink | undefined): void;
}

export class WorldClientRuntimeFacade implements ClientRuntime {
  private readonly clientWorld: ClientWorld;

  public constructor(private readonly client: WorldClient) {
    this.clientWorld = getClientWorld(client);
  }

  public openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage> {
    return this.client.openWorld(request);
  }

  public setChunkInterest(request: SetChunkViewRequest): Promise<boolean> {
    return this.client.setChunkView(request);
  }

  public sendPlayerCommand(request: SetPlayerInputRequest): Promise<boolean> {
    return this.client.setPlayerInput(request);
  }

  public drainTransportUpdates(): Promise<boolean> {
    return this.client.pollUpdates();
  }

  public setRenderWorldUpdateSink(sink: RenderWorldUpdateSink | undefined): void {
    if (isRenderWorldUpdateSinkTarget(this.client)) {
      this.client.setRenderWorldUpdateSink(sink);
      return;
    }

    throw new Error("WorldClientRuntimeFacade requires a render-world update sink target");
  }

  public getClientWorld(): ClientWorld {
    return this.clientWorld;
  }

  public publishPresentationState(): ClientPresentationState {
    return {
      sessionState: this.clientWorld.getSessionState(),
      localPlayerState: this.clientWorld.getLocalPlayerState(),
      entities: this.clientWorld.getEntitySnapshots(),
      performance: this.clientWorld.getPerformanceSnapshot(),
    };
  }
}

function getClientWorld(client: WorldClient): ClientWorld {
  if ("getClientWorld" in client && typeof client.getClientWorld === "function") {
    return client.getClientWorld() as ClientWorld;
  }

  return new WorldClientBackedClientWorld(client);
}

function isRenderWorldUpdateSinkTarget(client: WorldClient): client is WorldClient & RenderWorldUpdateSinkTarget {
  return "setRenderWorldUpdateSink" in client && typeof client.setRenderWorldUpdateSink === "function";
}
