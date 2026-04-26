import { createIntegratedServer } from "../src/runtime/host/integrated-server.ts";
import { WorkerWorldTransport, type WorldWorkerClientEndpoint } from "../src/runtime/transport/worker-world-transport.ts";
import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { getDefaultRenderDistance, getExpectedLoadedChunkCount } from "../src/renderer/browser-render-config.ts";
import {
  createGeneratedWorldHeadlessHarness,
  renderGeneratedWorldHeadlessFrame,
  type GeneratedWorldHeadlessHarness,
} from "../src/renderer/generated-world-headless-harness.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import { readPixel, renderFrameToHeadlessTarget } from "../src/renderer/static-frame-harness.ts";
import { Vec3 } from "../src/world/phys/vec3.ts";
import { createDenoExtractedAssetPack } from "./deno-file-asset-source.ts";
import { encodePngRgba } from "./png-rgba.ts";

const WIDTH = 256;
const HEIGHT = 256;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-generated-world-smoke.png";
const SEED = 12_345n;
const VIEW_DISTANCE = 1;
const RENDER_DISTANCE = getDefaultRenderDistance(VIEW_DISTANCE);
const MIN_NON_CLEAR_PIXELS = 1_000;
const CAMERA_STATE = {
  position: new Vec3(960.5, 132.0, -8127.5),
  xRot: 60.0,
  yRot: 225.0,
} as const;
const SKY_COLOR = new Vec3(0x8f / 255, 0xb8 / 255, 0xff / 255);

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
    seed: SEED,
    width: WIDTH,
    height: HEIGHT,
    viewDistance: VIEW_DISTANCE,
    renderDistance: RENDER_DISTANCE,
    preset: "browser_smoke",
    engineConfig: {
      lightingMode: "none",
      liquidSimulationMode: "none",
    },
    skyColor: SKY_COLOR,
    clearColorScale: 1,
  });

  const expectedLoadedChunkCount = getExpectedLoadedChunkCount(VIEW_DISTANCE);
  const frameResult = await renderGeneratedWorldHeadlessFrame(harness, CAMERA_STATE, expectedLoadedChunkCount);
  if (frameResult.solidDrawCount <= 0) {
    throw new Error("Deno generated-world smoke produced no solid chunk draws");
  }

  const readback = await renderFrameToHeadlessTarget({
    rendererHost,
    scene: harness.scene,
    frame: frameResult.frame,
    width: WIDTH,
    height: HEIGHT,
    format: FORMAT,
  });
  if (readback.nonClearPixels < MIN_NON_CLEAR_PIXELS) {
    throw new Error(`Deno generated-world smoke rendered too few non-clear pixels: ${readback.nonClearPixels.toString()}`);
  }
  const terrainPixel = readPixel(readback.pixels, readback.width, Math.floor(readback.width / 2), Math.floor((readback.height * 3) / 4));
  await Deno.writeFile(OUTPUT_PATH, encodePngRgba(readback.width, readback.height, readback.pixels));

  const presentation = harness.clientRuntime.publishPresentationState();
  console.log(JSON.stringify({
    ok: true,
    outputPath: OUTPUT_PATH,
    width: readback.width,
    height: readback.height,
    format: readback.format,
    saveId: harness.saveMetadata.saveId,
    expectedLoadedChunkCount,
    loadedChunkCount: frameResult.loadedChunkCount,
    viewDistance: VIEW_DISTANCE,
    renderDistance: RENDER_DISTANCE,
    solidDrawCount: frameResult.solidDrawCount,
    nonClearPixels: readback.nonClearPixels,
    centerPixel: Array.from(readback.centerPixel),
    terrainPixel: Array.from(terrainPixel),
    byteLength: readback.byteLength,
    renderWorldCounters: frameResult.renderWorldCounters,
    renderQueueStats: frameResult.renderQueueStats,
    sessionId: presentation.sessionState?.sessionId,
    playerId: presentation.sessionState?.playerId,
    playerTick: presentation.localPlayerState?.tick,
    playerPosition: presentation.localPlayerState
      ? [
        presentation.localPlayerState.position.x,
        presentation.localPlayerState.position.y,
        presentation.localPlayerState.position.z,
      ]
      : undefined,
    adapter: adapter.info ?? {},
  }));
} finally {
  harness?.close();
}
