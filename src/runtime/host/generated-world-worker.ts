import { normalizeWorldEngineConfig, type OpenWorldRequest } from "../protocol/world-messages";
import { createBrowserLightingService } from "../lighting/lighting-worker-client";
import { connectWorldWorkerSession, type WorldWorkerHostEndpoint } from "../transport/worker-world-transport";
import { IndexedDbWorldStorage } from "../storage/indexeddb-world-storage";
import { createGeneratedWorldHostForRequest } from "./generated-world-host-factory";
import { GeneratedWorldHost } from "./generated-world-host";

function createGeneratedWorldHost(request: OpenWorldRequest): GeneratedWorldHost {
  const worldStorage = request.storageMode === "none" || typeof indexedDB === "undefined"
    ? undefined
    : new IndexedDbWorldStorage(indexedDB);
  const engineConfig = normalizeWorldEngineConfig(request.config);
  return createGeneratedWorldHostForRequest(request, {
    chunkViewScheduling: "cooperative",
    worldStorage,
    lightingService: engineConfig.lightingMode === "none" ? undefined : createBrowserLightingService(),
  });
}

connectWorldWorkerSession(
  globalThis as unknown as WorldWorkerHostEndpoint,
  createGeneratedWorldHost,
);
