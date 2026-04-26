import type { OpenWorldRequest } from "../src/runtime/protocol/world-messages.ts";
import { connectWorldWorkerSession, type WorldWorkerHostEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import { createGeneratedWorldHostForRequest } from "../src/runtime/host/generated-world-host-factory.ts";

connectWorldWorkerSession(
  globalThis as unknown as WorldWorkerHostEndpoint,
  (request: OpenWorldRequest) => createGeneratedWorldHostForRequest(request, {
    chunkViewScheduling: "cooperative",
  }),
);
