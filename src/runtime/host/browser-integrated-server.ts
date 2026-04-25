import { WorkerWorldTransport, createGeneratedWorldWorker, type WorldWorkerClientEndpoint } from "../transport/worker-world-transport";
import { createIntegratedServer, type IntegratedServer } from "./integrated-server";

export function createBrowserIntegratedServer(
  endpoint: WorldWorkerClientEndpoint = createGeneratedWorldWorker(),
): IntegratedServer {
  // Browser worker: postMessage transport replaces vanilla's in-process memory channel.
  return createIntegratedServer(new WorkerWorldTransport(endpoint));
}
