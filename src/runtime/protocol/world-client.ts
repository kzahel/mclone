import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { OpenWorldRequest, SetChunkViewRequest, WorldOpenedMessage } from "./world-messages";

export interface WorldClient {
  openWorld(request?: OpenWorldRequest): Promise<WorldOpenedMessage>;

  setChunkView(request: SetChunkViewRequest): Promise<boolean>;

  getLevel(): ClientChunkCache;
}
