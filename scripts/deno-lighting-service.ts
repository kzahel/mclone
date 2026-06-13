import {
  LightingWorkerClient,
  type LightingWorkerClientEndpoint,
} from "../src/runtime/lighting/lighting-worker-client.ts";

export function createDenoLightingService(): LightingWorkerClient {
  return new LightingWorkerClient(new Worker(
    new URL("../src/runtime/lighting/lighting-worker.ts", import.meta.url),
    {
      type: "module",
      name: "mclone-deno-lighting-worker",
    },
  ) as unknown as LightingWorkerClientEndpoint);
}
