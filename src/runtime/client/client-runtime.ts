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
import { PlayerMovementPredictor } from "../movement";
import { type ClientWorld, type RenderWorldUpdateSink, WorldClientBackedClientWorld } from "./client-world";
import {
  PlayerMovementPredictionService,
  type PredictionService,
} from "./prediction-service";

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
  close(): void;
  getClientWorld(): ClientWorld;
  getPredictionService(): PredictionService;
  publishPresentationState(): ClientPresentationState;
}

interface RenderWorldUpdateSinkTarget {
  setRenderWorldUpdateSink(sink: RenderWorldUpdateSink | undefined): void;
}

interface ClientRuntimeCloseTarget {
  close(): void;
}

export class WorldClientRuntimeFacade implements ClientRuntime {
  private readonly clientWorld: ClientWorld;
  private predictionService: PredictionService | undefined;

  public constructor(private readonly client: WorldClient) {
    this.clientWorld = getClientWorld(client);
  }

  public async openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage> {
    const opened = await this.client.openWorld(request);
    this.predictionService = undefined;
    return opened;
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

  public close(): void {
    if (isRenderWorldUpdateSinkTarget(this.client)) {
      this.client.setRenderWorldUpdateSink(undefined);
    }
    if (isClientRuntimeCloseTarget(this.client)) {
      this.client.close();
    }
    this.predictionService = undefined;
  }

  public getClientWorld(): ClientWorld {
    return this.clientWorld;
  }

  public getPredictionService(): PredictionService {
    if (this.predictionService !== undefined) {
      return this.predictionService;
    }

    const authoritative = this.clientWorld.getPredictionView().getAuthoritativeMovementState();
    if (authoritative === undefined) {
      throw new Error("ClientRuntime prediction service requires an authoritative movement state");
    }

    const predictionService = new PlayerMovementPredictionService(
      new PlayerMovementPredictor(authoritative.body),
    );
    predictionService.resetFromAuthoritativeBody(
      authoritative.body,
      authoritative.lastProcessedCommandSeq,
    );
    this.predictionService = predictionService;
    return predictionService;
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

function isClientRuntimeCloseTarget(client: WorldClient): client is WorldClient & ClientRuntimeCloseTarget {
  return "close" in client && typeof client.close === "function";
}
