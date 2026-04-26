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
import { encodePngRgba } from "./png-rgba.ts";
import {
  DENO_VANILLA_ASSET_WORLD_HEIGHT,
  DENO_VANILLA_ASSET_WORLD_MIN_BUILD_HEIGHT,
  DENO_VANILLA_ASSET_WORLD_SEED,
  DENO_VANILLA_ASSET_WORLD_STONE_TEXTURE,
  DENO_VANILLA_ASSET_WORLD_VIEW_DISTANCE,
  createDenoVanillaAssetBlockRenderer,
  prepareDenoVanillaAssetAtlasResources,
} from "./deno-vanilla-asset-world-smoke-shared.ts";

const WIDTH = 128;
const HEIGHT = 128;
const FORMAT: GPUTextureFormat = "rgba8unorm";
const OUTPUT_PATH = "/tmp/mclone-deno-vanilla-asset-world-smoke.png";
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
  new URL("./deno-vanilla-asset-world-worker.ts", import.meta.url),
  {
    type: "module",
    name: "mclone-deno-vanilla-asset-world-worker",
  },
) as unknown as RenderWorldWorkerClientEndpoint);
const contextResult = await rendererHost.requestWebGpuDeviceContext();
if (!contextResult.ok) {
  throw new Error(contextResult.reason);
}

const { adapter, device } = contextResult.context;
const blocks = registerGeneratedRenderBlocks();
const atlas = new TextureAtlas(TextureAtlas.LOCATION_BLOCKS, device.limits.maxTextureDimension2D);
const assetResources = await prepareDenoVanillaAssetAtlasResources(atlas);
atlas.reload(device, assetResources.preparations);
const stoneSprite = atlas.getSprite(DENO_VANILLA_ASSET_WORLD_STONE_TEXTURE);

let frameHarness: StaticRendererFrameHarness | undefined;
try {
  const blockRenderer = await createDenoVanillaAssetBlockRenderer(
    assetResources.assetPack,
    blocks,
    (location) => atlas.getSprite(location),
  );
  frameHarness = await createStaticRendererFrameHarness({
    device,
    rendererHost,
    atlas,
    blocks,
    blockRenderer,
    snapshots: createStoneWallChunkSnapshotMessages({
      blocks,
      seed: DENO_VANILLA_ASSET_WORLD_SEED,
      minBuildHeight: DENO_VANILLA_ASSET_WORLD_MIN_BUILD_HEIGHT,
      worldHeight: DENO_VANILLA_ASSET_WORLD_HEIGHT,
      skyColor: SKY_COLOR,
    }),
    seed: DENO_VANILLA_ASSET_WORLD_SEED,
    minBuildHeight: DENO_VANILLA_ASSET_WORLD_MIN_BUILD_HEIGHT,
    worldHeight: DENO_VANILLA_ASSET_WORLD_HEIGHT,
    width: WIDTH,
    height: HEIGHT,
    viewDistance: DENO_VANILLA_ASSET_WORLD_VIEW_DISTANCE,
    renderDistance: RENDER_DISTANCE,
    skyColor: SKY_COLOR,
  });

  const frameResult = await renderStaticRendererFrame(frameHarness, CAMERA_STATE);
  if (frameResult.drawCount <= 0) {
    throw new Error("Deno vanilla asset world smoke produced no chunk draws");
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
    throw new Error(`Deno vanilla asset world smoke rendered too few non-clear pixels: ${readback.nonClearPixels.toString()}`);
  }
  assertStoneTextureLike(readback.centerPixel);
  await Deno.writeFile(OUTPUT_PATH, encodePngRgba(readback.width, readback.height, readback.pixels));

  console.log(JSON.stringify({
    ok: true,
    outputPath: OUTPUT_PATH,
    width: readback.width,
    height: readback.height,
    format: readback.format,
    atlasWidth: assetResources.preparations.width,
    atlasHeight: assetResources.preparations.height,
    mipLevel: assetResources.preparations.mipLevel,
    stoneTexture: DENO_VANILLA_ASSET_WORLD_STONE_TEXTURE.toString(),
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
  assetResources.atlasSource.close();
}

function assertStoneTextureLike(pixel: Uint8Array): void {
  const [red, green, blue, alpha] = pixel;
  if (alpha !== 255 || red === undefined || green === undefined || blue === undefined) {
    throw new Error(`Deno vanilla asset center pixel is not opaque RGBA: ${Array.from(pixel).join(",")}`);
  }

  const maxChannel = Math.max(red, green, blue);
  const minChannel = Math.min(red, green, blue);
  if (maxChannel - minChannel > 18 || minChannel < 40 || maxChannel > 220) {
    throw new Error(`Deno vanilla asset center pixel does not look like the stone texture: ${Array.from(pixel).join(",")}`);
  }
}
