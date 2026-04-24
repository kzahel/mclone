import { BlockPos } from "../../core/block-pos";
import { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { ChunkSnapshot } from "../../world/level/chunk-snapshot";
import { ChunkBufferBuilderPack } from "../chunk-buffer-builder-pack";
import {
  serializeSectionMeshBuild,
  type BuildSectionMeshRequest,
  type MeshWorkerRequest,
  type MeshWorkerResponse,
} from "./chunk-mesh-protocol";
import { createBrowserChunkMeshContext, type BrowserChunkMeshContext } from "./mesh-worker-context";
import { connectMeshWorkerSession, type MeshWorkerHostEndpoint } from "./mesh-worker-client";
import { RenderChunkRegion } from "./render-chunk-region";
import { buildSectionMesh } from "./section-mesh-compiler";
import { Vec3 } from "../../world/phys/vec3";

function createJobLevel(context: BrowserChunkMeshContext, snapshots: readonly ChunkSnapshot[]): ClientChunkCache {
  const level = new ClientChunkCache({
    airState: context.airState,
    minBuildHeight: context.minBuildHeight,
    height: context.height,
    biomeSource: context.biomeSource,
    biomeZoomSeed: context.seed,
    blockStateResolver: context.blockStateResolver,
    blockStateIds: context.blockStateIds,
  });

  for (const snapshot of snapshots) {
    level.applyChunkSnapshot(snapshot);
  }

  return level;
}

async function buildWorkerResponse(context: BrowserChunkMeshContext, request: BuildSectionMeshRequest): Promise<MeshWorkerResponse> {
  const level = createJobLevel(context, request.snapshots);
  const origin = new BlockPos(request.origin.x, request.origin.y, request.origin.z);
  const region = RenderChunkRegion.createIfNotEmpty(level, origin.offset(-1, -1, -1), origin.offset(16, 16, 16), 1);
  const buffers = new ChunkBufferBuilderPack();

  try {
    const build = buildSectionMesh(
      origin,
      new Vec3(request.camera.x, request.camera.y, request.camera.z),
      region,
      context.blockRenderer,
      buffers,
    );

    return {
      type: "section_mesh_built",
      result: serializeSectionMeshBuild(build, buffers),
    };
  } finally {
    buffers.discardAll();
  }
}

let meshWorkerContextPromise: Promise<BrowserChunkMeshContext> | undefined;

async function handleMeshWorkerRequest(message: MeshWorkerRequest): Promise<MeshWorkerResponse> {
  switch (message.type) {
    case "initialize_mesh_worker":
      meshWorkerContextPromise = createBrowserChunkMeshContext(message);
      await meshWorkerContextPromise;
      return { type: "mesh_worker_ready" };
    case "build_section_mesh": {
      if (meshWorkerContextPromise === undefined) {
        throw new Error("build_section_mesh received before initialize_mesh_worker");
      }

      return buildWorkerResponse(await meshWorkerContextPromise, message);
    }
  }
}

connectMeshWorkerSession(
  globalThis as unknown as MeshWorkerHostEndpoint,
  handleMeshWorkerRequest,
);
