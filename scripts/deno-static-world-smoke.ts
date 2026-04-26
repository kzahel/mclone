import type { RenderWorldWorkerClientEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { createHeadlessRendererHost } from "../src/renderer/renderer-host.ts";
import {
  createStaticRendererFrameHarness,
  createStoneWallChunkSnapshotMessages,
  renderFrameToHeadlessTarget,
  renderStaticRendererFrame,
  type StaticRendererFrameHarness,
} from "../src/renderer/static-frame-harness.ts";
import { TextureAtlas } from "../src/renderer/texture/texture-atlas.ts";
import { registerGeneratedRenderBlocks } from "../src/world/level/generated-render-blocks.ts";
import { Vec3 } from "../src/world/phys/vec3.ts";
import { ChunkBlockId } from "../src/worldgen/chunk/chunk-block-buffer.ts";
import { encodePngRgba } from "./png-rgba.ts";
import {
  DENO_STATIC_WORLD_HEIGHT,
  DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
  DENO_STATIC_WORLD_SEED,
  DENO_STATIC_WORLD_STONE_TEXTURE,
  DENO_STATIC_WORLD_TEXTURE_MIP_LEVEL,
  DENO_STATIC_WORLD_VIEW_DISTANCE,
  DenoStaticWorldTextureAtlasSource,
  createDenoStaticWorldBlockRenderer,
} from "./deno-static-world-smoke-shared.ts";

const WIDTH = 128;
const HEIGHT = 128;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-static-world-smoke.png";
const CAMERA_POSITION = new Vec3(8.5, 8.5, 28.0);
const CAMERA_STATE = {
  position: CAMERA_POSITION,
  xRot: 0,
  yRot: 180,
} as const;
const SKY_COLOR = new Vec3(0.55, 0.7, 1.0);
const RENDER_DISTANCE = 128;
const MIN_NON_CLEAR_PIXELS = 1_000;

const rendererHost = createHeadlessRendererHost(() => new Worker(
  new URL("./deno-static-world-worker.ts", import.meta.url),
  {
    type: "module",
    name: "mclone-deno-static-world-worker",
  },
) as unknown as RenderWorldWorkerClientEndpoint);
const contextResult = await rendererHost.requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const blocks = registerGeneratedRenderBlocks();
const source = new DenoStaticWorldTextureAtlasSource();
const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS, device.limits.maxTextureDimension2D);
const preparations = await atlas.prepareToStitch(source, [DENO_STATIC_WORLD_STONE_TEXTURE], DENO_STATIC_WORLD_TEXTURE_MIP_LEVEL);
atlas.reload(device, preparations);
const stoneSprite = atlas.getSprite(DENO_STATIC_WORLD_STONE_TEXTURE);

let frameHarness: StaticRendererFrameHarness | undefined;
try {
  const blockRenderer = createDenoStaticWorldBlockRenderer(
    stoneSprite,
    blocks.blockStateById[ChunkBlockId.WATER]!,
    blocks.blockStateById[ChunkBlockId.LAVA]!,
  );
  frameHarness = await createStaticRendererFrameHarness({
    device,
    rendererHost,
    atlas,
    blocks,
    blockRenderer,
    snapshots: createStoneWallChunkSnapshotMessages({
      blocks,
      seed: DENO_STATIC_WORLD_SEED,
      minBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
      worldHeight: DENO_STATIC_WORLD_HEIGHT,
      skyColor: SKY_COLOR,
    }),
    seed: DENO_STATIC_WORLD_SEED,
    minBuildHeight: DENO_STATIC_WORLD_MIN_BUILD_HEIGHT,
    worldHeight: DENO_STATIC_WORLD_HEIGHT,
    width: WIDTH,
    height: HEIGHT,
    viewDistance: DENO_STATIC_WORLD_VIEW_DISTANCE,
    renderDistance: RENDER_DISTANCE,
    skyColor: SKY_COLOR,
  });

  const frameResult = await renderStaticRendererFrame(frameHarness, CAMERA_STATE);
  if (frameResult.drawCount <= 0) {
    throw new Error("Deno static-world smoke produced no chunk draws");
  }

  const readback = await renderFrameToHeadlessTarget({
    rendererHost,
    scene: frameHarness.scene,
    frame: frameResult.frame,
    width: WIDTH,
    height: HEIGHT,
    format: FORMAT,
  });
  if (readback.nonClearPixels < MIN_NON_CLEAR_PIXELS) {
    throw new Error(`Deno static-world smoke rendered too few non-clear pixels: ${readback.nonClearPixels.toString()}`);
  }
  assertStoneLike(readback.centerPixel);
  await Deno.writeFile(OUTPUT_PATH, encodePngRgba(readback.width, readback.height, readback.pixels));

  console.log(JSON.stringify({
    ok: true,
    outputPath: OUTPUT_PATH,
    width: readback.width,
    height: readback.height,
    format: readback.format,
    atlasWidth: preparations.width,
    atlasHeight: preparations.height,
    stoneSprite: {
      u0: stoneSprite.getU0(),
      u1: stoneSprite.getU1(),
      v0: stoneSprite.getV0(),
      v1: stoneSprite.getV1(),
    },
    dirtySectionCount: frameHarness.dirtySectionCount,
    renderedChunkCount: frameResult.renderedChunkCount,
    solidDrawCount: frameResult.drawCount,
    nonClearPixels: readback.nonClearPixels,
    centerPixel: Array.from(readback.centerPixel),
    byteLength: readback.byteLength,
    workerCounters: frameResult.workerCounters,
    adapter: adapter.info ?? {},
  }));
} finally {
  frameHarness?.close();
  atlas.clearTextureData();
  source.close();
}

function assertStoneLike(pixel: Uint8Array): void {
  const [red, green, blue, alpha] = pixel;
  if (alpha !== 255 || red === undefined || green === undefined || blue === undefined) {
    throw new Error(`Deno static-world center pixel is not opaque RGBA: ${Array.from(pixel).join(",")}`);
  }

  const maxChannel = Math.max(red, green, blue);
  const minChannel = Math.min(red, green, blue);
  if (maxChannel - minChannel > 12 || minChannel < 64 || maxChannel > 220) {
    throw new Error(`Deno static-world center pixel does not look like the stone wall: ${Array.from(pixel).join(",")}`);
  }
}
