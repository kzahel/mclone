import type { OpenWorldRequest } from "../src/runtime/protocol/world-messages.ts";
import { connectWorldWorkerSession, type WorldWorkerHostEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import { createGeneratedWorldHostForRequest } from "../src/runtime/host/generated-world-host-factory.ts";
import { normalizeWorldEngineConfig } from "../src/runtime/protocol/world-messages.ts";
import { createDenoLightingService } from "./deno-lighting-service.ts";

connectWorldWorkerSession(
  globalThis as unknown as WorldWorkerHostEndpoint,
  (request: OpenWorldRequest) => {
    const engineConfig = normalizeWorldEngineConfig(request.config);
    return createGeneratedWorldHostForRequest(request, {
      chunkViewScheduling: "cooperative",
      lightingService: engineConfig.lightingMode === "none" ? undefined : createDenoLightingService(),
    });
  },
);
