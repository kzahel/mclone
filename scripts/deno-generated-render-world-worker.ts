import { connectRenderWorldWorkerSession, type RenderWorldWorkerHostEndpoint } from "../src/renderer/chunk/render-world-worker-client.ts";
import { createAssetPackRenderWorldContext } from "../src/renderer/chunk/asset-pack-mesh-context.ts";
import { createRenderWorldWorkerHandler } from "../src/renderer/chunk/render-world-worker-handler.ts";
import { createDenoExtractedAssetPack } from "./deno-file-asset-source.ts";

connectRenderWorldWorkerSession(
  globalThis as unknown as RenderWorldWorkerHostEndpoint,
  createRenderWorldWorkerHandler((request, onProgress) => createAssetPackRenderWorldContext(
    createDenoExtractedAssetPack(),
    request,
    onProgress,
  )),
);
