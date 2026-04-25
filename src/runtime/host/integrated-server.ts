import type { WorldHost } from "../protocol/world-host";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
} from "../protocol/world-messages";

export type DedicatedServerHost = WorldHost;

export interface IntegratedServer extends WorldHost {
  readonly runtimeKind: "integrated_server";
  readonly authority: WorldHost;
}

export class IntegratedServerFacade implements IntegratedServer {
  public readonly runtimeKind = "integrated_server";

  public constructor(public readonly authority: WorldHost) {}

  public openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    return this.authority.openWorld(request);
  }

  public setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    return this.authority.setChunkView(request);
  }

  public setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    return this.authority.setPlayerInput(request);
  }

  public pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    return this.authority.pollUpdates(request);
  }

  public close(): void {
    this.authority.close?.();
  }
}

export function createIntegratedServer(authority: WorldHost): IntegratedServer {
  return new IntegratedServerFacade(authority);
}
