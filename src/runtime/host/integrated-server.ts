import type { WorldHost } from "../protocol/world-host";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
} from "../protocol/world-messages";

export type DedicatedServerHost = WorldHost;
export type IntegratedServerLifecycleState = "created" | "running" | "paused" | "closed";

export interface IntegratedServer extends WorldHost {
  readonly runtimeKind: "integrated_server";
  readonly authority: WorldHost;
  getLifecycleState(): IntegratedServerLifecycleState;
  isReady(): boolean;
  isPaused(): boolean;
  isClosed(): boolean;
  pause(): void;
  resume(): void;
  close(): void;
}

export class IntegratedServerFacade implements IntegratedServer {
  public readonly runtimeKind = "integrated_server";
  private lifecycleState: IntegratedServerLifecycleState = "created";

  public constructor(public readonly authority: WorldHost) {}

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.assertOpen("openWorld");
    const messages = await this.authority.openWorld(request);
    if (this.lifecycleState !== "closed") {
      this.lifecycleState = "running";
    }
    return messages;
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    this.assertOpen("setChunkView");
    return this.authority.setChunkView(request);
  }

  public setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    this.assertOpen("setPlayerInput");
    return this.authority.setPlayerInput(request);
  }

  public pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    this.assertOpen("pollUpdates");
    return this.authority.pollUpdates(request);
  }

  public getLifecycleState(): IntegratedServerLifecycleState {
    return this.lifecycleState;
  }

  public isReady(): boolean {
    return this.lifecycleState === "running" || this.lifecycleState === "paused";
  }

  public isPaused(): boolean {
    return this.lifecycleState === "paused";
  }

  public isClosed(): boolean {
    return this.lifecycleState === "closed";
  }

  public pause(): void {
    this.assertOpen("pause");
    if (this.lifecycleState === "running") {
      this.lifecycleState = "paused";
    }
  }

  public resume(): void {
    this.assertOpen("resume");
    if (this.lifecycleState === "paused") {
      this.lifecycleState = "running";
    }
  }

  public close(): void {
    if (this.lifecycleState === "closed") {
      return;
    }

    this.lifecycleState = "closed";
    this.authority.close?.();
  }

  private assertOpen(methodName: string): void {
    if (this.lifecycleState === "closed") {
      throw new Error(`IntegratedServer.${methodName}() called after close()`);
    }
  }
}

export function createIntegratedServer(authority: WorldHost): IntegratedServer {
  return new IntegratedServerFacade(authority);
}
