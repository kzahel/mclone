import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { ClientPlayerState, ClientSessionState, OpenWorldRequest, SetChunkViewRequest, SetPlayerInputRequest, WorldOpenedMessage } from "./world-messages";

export interface WorldClient {
  openWorld(request: OpenWorldRequest): Promise<WorldOpenedMessage>;

  setChunkView(request: SetChunkViewRequest): Promise<boolean>;

  setPlayerInput(request: SetPlayerInputRequest): Promise<boolean>;

  pollUpdates(): Promise<boolean>;

  getLevel(): ClientChunkCache;

  getSessionState(): ClientSessionState | undefined;

  getPlayerState(): ClientPlayerState | undefined;
}
