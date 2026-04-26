import { createIntegratedServer } from "../src/runtime/host/integrated-server.ts";
import { WorkerWorldTransport, type WorldWorkerClientEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import {
  createGeneratedWorldHeadlessHarness,
  type GeneratedWorldHeadlessHarness,
} from "../src/renderer/generated-world-headless-harness.ts";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TICK_CADENCE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  type GeneratedWorldSmokeScenario,
  validateGeneratedWorldSmokeResult,
} from "../src/renderer/generated-world-smoke-scenario.ts";
import {
  runGeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeRun,
  type GeneratedWorldSmokeScenarioResult,
} from "../src/renderer/generated-world-smoke-runner.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import { renderFrameToHeadlessTarget } from "../src/renderer/static-frame-harness.ts";
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

  let harness: GeneratedWorldHeadlessHarness | undefined;
  try {
    harness = await createGeneratedWorldHeadlessHarness({
      adapter,
      device,
      format,
      rendererHost,
      worldTransport,
      assetPack: createDenoExtractedAssetPack(),
      seed: scenario.seed,
      width: scenario.width,
      height: scenario.height,
      viewDistance: scenario.viewDistance,
      renderDistance: scenario.renderDistance,
      preset: scenario.preset,
      engineConfig: scenario.engineConfig,
      skyColor: scenario.skyColor,
      clearColorScale: scenario.clearColorScale,
    });

    const run = await runGeneratedWorldSmokeScenario({
      scene: harness.scene,
      scenario,
      worldTransport: "worker",
      target: {
        width: scenario.width,
        height: scenario.height,
        format: scenario.renderTargetFormat,
        renderFrame: ({ scene, frame }) => renderFrameToHeadlessTarget({
          rendererHost,
          scene,
          frame,
          width: scenario.width,
          height: scenario.height,
          format: scenario.renderTargetFormat,
        }),
      },
    });

    await writeScenarioArtifacts(run);

    const smokeResult = {
      ...run.result,
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
    harness?.close();
  }
}

async function writeScenarioArtifacts(run: GeneratedWorldSmokeRun): Promise<void> {
  for (const stepRun of run.stepRuns) {
    const outputPath = stepRun.result.outputPath;
    if (outputPath === undefined) {
      continue;
    }
    if (stepRun.readback.pixels === undefined) {
      throw new Error(`Deno generated-world step ${stepRun.result.stepName} did not produce readback pixels`);
    }
    await Deno.writeFile(outputPath, encodePngRgba(
      stepRun.readback.width,
      stepRun.readback.height,
      stepRun.readback.pixels,
    ));
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
