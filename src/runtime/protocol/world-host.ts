import type { OpenWorldRequest, SetChunkViewRequest, WorldHostMessage } from "./world-messages";

export interface WorldHost {
  openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]>;

  setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]>;
}
