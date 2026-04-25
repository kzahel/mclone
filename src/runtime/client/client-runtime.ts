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
import { type ClientWorld, WorldClientBackedClientWorld } from "./client-world";

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
  getClientWorld(): ClientWorld;
  publishPresentationState(): ClientPresentationState;
}

export class WorldClientRuntimeFacade implements ClientRuntime {
  private readonly clientWorld: ClientWorld;

  public constructor(private readonly client: WorldClient) {
    this.clientWorld = new WorldClientBackedClientWorld(client);
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

  public getClientWorld(): ClientWorld {
    return this.clientWorld;
  }

  public publishPresentationState(): ClientPresentationState {
    return {
      sessionState: this.client.getSessionState(),
      localPlayerState: this.client.getPlayerState(),
      entities: this.client.getEntitySnapshots(),
      performance: this.client.getPerformanceSnapshot(),
    };
  }
}
