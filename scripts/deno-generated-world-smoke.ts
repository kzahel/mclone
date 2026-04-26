import { createIntegratedServer } from "../src/runtime/host/integrated-server.ts";
import { WorkerWorldTransport, type WorldWorkerClientEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import {
  createGeneratedWorldHeadlessHarness,
  renderGeneratedWorldHeadlessFrame,
  type GeneratedWorldHeadlessHarness,
} from "../src/renderer/generated-world-headless-harness.ts";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  getGeneratedWorldSmokeExpectedLoadedChunkCount,
  runGeneratedWorldSmokePlayerInput,
  validateGeneratedWorldSmokeResult,
} from "../src/renderer/generated-world-smoke-scenario.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import { readPixel, renderFrameToHeadlessTarget } from "../src/renderer/static-frame-harness.ts";
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

  const expectedLoadedChunkCount = getGeneratedWorldSmokeExpectedLoadedChunkCount(SCENARIO);
  const frameResult = await renderGeneratedWorldHeadlessFrame(harness, SCENARIO.camera, expectedLoadedChunkCount);

  const readback = await renderFrameToHeadlessTarget({
    rendererHost,
    scene: harness.scene,
    frame: frameResult.frame,
    width: SCENARIO.width,
    height: SCENARIO.height,
    format: SCENARIO.renderTargetFormat,
  });
  const terrainPixel = readPixel(readback.pixels, readback.width, Math.floor(readback.width / 2), Math.floor((readback.height * 3) / 4));
  await Deno.writeFile(SCENARIO.outputPath, encodePngRgba(readback.width, readback.height, readback.pixels));

  await runGeneratedWorldSmokePlayerInput(harness.scene, SCENARIO);
  const presentation = harness.clientRuntime.publishPresentationState();
  const smokeResult = {
    ok: true,
    outputPath: SCENARIO.outputPath,
    worldTransport: "worker" as const,
    width: readback.width,
    height: readback.height,
    format: readback.format,
    saveId: harness.saveMetadata.saveId,
    expectedLoadedChunkCount,
    loadedChunkCount: frameResult.loadedChunkCount,
    viewDistance: SCENARIO.viewDistance,
    renderDistance: SCENARIO.renderDistance,
    lightingMode: SCENARIO.engineConfig.lightingMode,
    liquidSimulationMode: SCENARIO.engineConfig.liquidSimulationMode,
    solidDrawCount: frameResult.solidDrawCount,
    nonClearPixels: readback.nonClearPixels,
    centerPixel: Array.from(readback.centerPixel),
    terrainPixel: Array.from(terrainPixel),
    byteLength: readback.byteLength,
    renderWorldCounters: frameResult.renderWorldCounters,
    renderQueueStats: frameResult.renderQueueStats,
    sessionId: presentation.sessionState?.sessionId,
    playerId: presentation.sessionState?.playerId,
    playerName: presentation.sessionState?.playerProfile.name,
    playerProfileId: presentation.sessionState?.playerProfile.profileId,
    sessionRevision: presentation.sessionState?.revision,
    playerInputSequence: presentation.localPlayerState?.acknowledgedInputSequence,
    playerStateRevision: presentation.localPlayerState?.revision,
    playerTick: presentation.localPlayerState?.tick,
    playerPosition: presentation.localPlayerState
      ? [
        presentation.localPlayerState.position.x,
        presentation.localPlayerState.position.y,
        presentation.localPlayerState.position.z,
      ]
      : undefined,
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
