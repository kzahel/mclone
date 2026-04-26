import { createIntegratedServer } from "../src/runtime/host/integrated-server.ts";
import { WorkerWorldTransport, type WorldWorkerClientEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { runGeneratedWorldBoot } from "../src/renderer/generated-world-boot.ts";
import { createGeneratedWorldHeadlessBootAdapter } from "../src/renderer/generated-world-headless-boot.ts";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TICK_CADENCE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  type GeneratedWorldSmokeScenario,
  validateGeneratedWorldSmokeResult,
} from "../src/renderer/generated-world-smoke-scenario.ts";
import {
  type GeneratedWorldSmokeScenarioResult,
} from "../src/renderer/generated-world-smoke-runner.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import { createDenoExtractedAssetPack } from "./deno-file-asset-source.ts";
import { encodePngRgba } from "./png-rgba.ts";

const rendererHost = createHeadlessRendererHost(() => new Worker(
  new URL("./deno-generated-render-world-worker.ts", import.meta.url),
  {
    type: "module",
    name: "mclone-deno-generated-render-world-worker",
  },
) as unknown as RenderWorldWorkerClientEndpoint);
const contextResult = await rendererHost.requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device, format } = contextResult.context;

type DenoGeneratedWorldScenarioResult = GeneratedWorldSmokeScenarioResult & {
  readonly adapter: GPUAdapterInfo | Record<string, never>;
};

async function runDenoGeneratedWorldScenario(
  scenario: GeneratedWorldSmokeScenario,
): Promise<DenoGeneratedWorldScenarioResult> {
  const worldTransport = createIntegratedServer(new WorkerWorldTransport(
    new Worker(new URL("./deno-generated-world-worker.ts", import.meta.url), {
      type: "module",
      name: "mclone-deno-generated-world-worker",
    }) as unknown as WorldWorkerClientEndpoint,
  ));

  const bootResult = await runGeneratedWorldBoot({
    scenario,
    worldTransport: "worker",
    adapter: createGeneratedWorldHeadlessBootAdapter({
      gpuAdapter: adapter,
      device,
      format,
      rendererHost,
      worldTransport,
      assetPack: createDenoExtractedAssetPack(),
      writePngArtifact: async ({ outputPath, width, height, pixels }) => {
        await Deno.writeFile(outputPath, encodePngRgba(width, height, pixels));
      },
    }),
  });
  if (!bootResult.ok) {
    throw new Error(bootResult.reason);
  }

  try {
    const smokeResult = {
      ...bootResult.run.result,
      adapter: adapter.info ?? {},
    };
    const validationErrors = validateGeneratedWorldSmokeResult(smokeResult, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, scenario);
    if (validationErrors.length > 0) {
      throw new Error(`Deno generated-world scenario ${scenario.id} failed validation:\n${validationErrors.join("\n")}`);
    }

    return smokeResult;
  } finally {
    await bootResult.close();
  }
}

const smokeResult = await runDenoGeneratedWorldScenario(GENERATED_WORLD_SMOKE_SCENARIO);
const transitionResult = await runDenoGeneratedWorldScenario(GENERATED_WORLD_TRANSITION_SCENARIO);
const tickCadenceResult = await runDenoGeneratedWorldScenario(GENERATED_WORLD_TICK_CADENCE_SCENARIO);

console.log(JSON.stringify({
  ...smokeResult,
  transition: transitionResult,
  tickCadence: tickCadenceResult,
}));
