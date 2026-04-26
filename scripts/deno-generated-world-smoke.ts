import { createIntegratedServer } from "../src/runtime/host/integrated-server.ts";
import { WorkerWorldTransport, type WorldWorkerClientEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import {
  createGeneratedWorldHeadlessHarness,
  type GeneratedWorldHeadlessHarness,
} from "../src/renderer/generated-world-headless-harness.ts";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  validateGeneratedWorldSmokeResult,
} from "../src/renderer/generated-world-smoke-scenario.ts";
import { runGeneratedWorldSmokeScenario } from "../src/renderer/generated-world-smoke-runner.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import { renderFrameToHeadlessTarget } from "../src/renderer/static-frame-harness.ts";
import { createDenoExtractedAssetPack } from "./deno-file-asset-source.ts";
import { encodePngRgba } from "./png-rgba.ts";

const SCENARIO = GENERATED_WORLD_SMOKE_SCENARIO;

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
const worldTransport = createIntegratedServer(new WorkerWorldTransport(
  new Worker(new URL("./deno-generated-world-worker.ts", import.meta.url), {
    type: "module",
    name: "mclone-deno-generated-world-worker",
  }) as unknown as WorldWorkerClientEndpoint,
));

let harness: GeneratedWorldHeadlessHarness | undefined;
try {
  harness = await createGeneratedWorldHeadlessHarness({
    adapter,
    device,
    format,
    rendererHost,
    worldTransport,
    assetPack: createDenoExtractedAssetPack(),
    seed: SCENARIO.seed,
    width: SCENARIO.width,
    height: SCENARIO.height,
    viewDistance: SCENARIO.viewDistance,
    renderDistance: SCENARIO.renderDistance,
    preset: SCENARIO.preset,
    engineConfig: SCENARIO.engineConfig,
    skyColor: SCENARIO.skyColor,
    clearColorScale: SCENARIO.clearColorScale,
  });

  const run = await runGeneratedWorldSmokeScenario({
    scene: harness.scene,
    scenario: SCENARIO,
    worldTransport: "worker",
    outputPath: SCENARIO.outputPath,
    target: {
      width: SCENARIO.width,
      height: SCENARIO.height,
      format: SCENARIO.renderTargetFormat,
      renderFrame: ({ scene, frame }) => renderFrameToHeadlessTarget({
        rendererHost,
        scene,
        frame,
        width: SCENARIO.width,
        height: SCENARIO.height,
        format: SCENARIO.renderTargetFormat,
      }),
    },
  });
  if (run.readback.pixels === undefined) {
    throw new Error("Deno generated-world smoke did not produce readback pixels");
  }
  await Deno.writeFile(SCENARIO.outputPath, encodePngRgba(run.readback.width, run.readback.height, run.readback.pixels));

  const smokeResult = {
    ...run.result,
    adapter: adapter.info ?? {},
  };
  const validationErrors = validateGeneratedWorldSmokeResult(smokeResult, {
    expectedWorldTransport: "worker",
    requireReadback: true,
    requirePlayerInput: true,
  }, SCENARIO);
  if (validationErrors.length > 0) {
    throw new Error(`Deno generated-world smoke failed validation:\n${validationErrors.join("\n")}`);
  }

  console.log(JSON.stringify(smokeResult));
} finally {
  harness?.close();
}
