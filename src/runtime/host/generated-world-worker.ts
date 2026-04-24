import type { OpenWorldRequest } from "../protocol/world-messages";
import { createBrowserLightingService } from "../lighting/lighting-worker-client";
import { connectWorldWorkerSession, type WorldWorkerHostEndpoint } from "../transport/worker-world-transport";
import { IndexedDbWorldStorage } from "../storage/indexeddb-world-storage";
import { createGeneratedWorldHostForRequest } from "./generated-world-host-factory";
import { GeneratedWorldHost } from "./generated-world-host";

function createGeneratedWorldHost(request: OpenWorldRequest): GeneratedWorldHost {
  return createGeneratedWorldHostForRequest(request, {
    chunkViewScheduling: "cooperative",
    worldStorage: typeof indexedDB === "undefined" ? undefined : new IndexedDbWorldStorage(indexedDB),
    lightingService: request.config?.lightingMode === "none" ? undefined : createBrowserLightingService(),
  });
}

connectWorldWorkerSession(
  globalThis as unknown as WorldWorkerHostEndpoint,
  createGeneratedWorldHost,
);
