import type { OpenWorldRequest, PollWorldUpdatesRequest, SetChunkViewRequest, SetPlayerInputRequest, WorldHostMessage } from "./world-messages";

export interface WorldHost {
  openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]>;

  setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]>;

  setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]>;

  pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]>;

  close?(): void;
}
