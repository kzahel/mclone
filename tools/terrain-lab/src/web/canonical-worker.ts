/// <reference lib="webworker" />

import initTerrainLab, {
  CanonicalTerrainMeshSession,
} from "../../generated/pkg/mclone_terrain_lab";

import type {
  CanonicalWorkerBatchResult,
  CanonicalWorkerRequest,
  CanonicalWorkerResponse,
} from "./canonical-worker-protocol";

const AUTHORED_PACK_URL = "/first-party-packs/mclone-authored.pbp";
const FALLBACK_PACK_URL = "/first-party-packs/mclone-generated-fallback.pbp";

let session: CanonicalTerrainMeshSession | undefined;
let activeEpoch = 0;

self.onmessage = (event: MessageEvent<CanonicalWorkerRequest>): void => {
  const request = event.data;
  if (request.type === "init") {
    activeEpoch = request.epoch;
    session?.free();
    session = undefined;
    void Promise.all([
      initTerrainLab(),
      fetchPack(AUTHORED_PACK_URL),
      fetchPack(FALLBACK_PACK_URL),
    ])
      .then(([, authored, fallback]) => {
        if (activeEpoch !== request.epoch) {
          return;
        }
        session = CanonicalTerrainMeshSession.withProfile(
          authored,
          fallback,
          request.seed,
          request.profile,
          request.stage,
        );
        post({ type: "ready", epoch: request.epoch });
      })
      .catch((error: unknown) => {
        post({
          type: "error",
          epoch: request.epoch,
          message: errorMessage(error),
        });
      });
    return;
  }
  if (!session) {
    post({
      type: "error",
      epoch: request.epoch,
      message: "Canonical terrain mesh Worker is not initialized.",
    });
    return;
  }
  if (request.type === "begin") {
    activeEpoch = request.epoch;
    try {
      session.begin(
        JSON.stringify(request.coordinates),
        request.waterVisible,
        request.vegetationVisible,
        request.cacheEnabled,
      );
      post({
        type: "began",
        epoch: request.epoch,
        rawCacheChunks: session.rawCacheChunks,
        rawCacheBytes: Number(session.rawCacheBytes),
      });
    } catch (error: unknown) {
      post({
        type: "error",
        epoch: request.epoch,
        message: errorMessage(error),
      });
    }
    return;
  }
  if (request.epoch !== activeEpoch) {
    return;
  }
  try {
    const payload = session.compileBatch(JSON.stringify(request.coordinates));
    const transferStarted = performance.now();
    const admissions = Array.from(
      { length: payload.admissionCount },
      (_, index) => ({
        chunkX: payload.admissionChunkX(index),
        chunkZ: payload.admissionChunkZ(index),
        fingerprint: payload.admissionFingerprint(index),
        rawCacheHit: payload.admissionRawCacheHit(index),
        dependencyCacheHits: payload.admissionDependencyCacheHits(index),
        generatedDependencyChunks:
          payload.admissionGeneratedDependencyChunks(index),
        retainedDependencyChunks:
          payload.admissionRetainedDependencyChunks(index),
        targetChunks: payload.admissionTargetChunks(index),
        sectionCount: payload.admissionSectionCount(index),
        vertexCount: payload.admissionVertexCount(index),
        indexCount: payload.admissionIndexCount(index),
        packedSections: payload.admissionPackedSections(index),
      }),
    );
    const result: CanonicalWorkerBatchResult = {
      type: "batch",
      epoch: request.epoch,
      generationMs: payload.generationMs,
      presentationMs: payload.presentationMs,
      meshMs: payload.meshMs,
      packMs: payload.packMs,
      transferMs: performance.now() - transferStarted,
      deduplicatedTargetChunks: payload.deduplicatedTargetChunks,
      rawCacheChunks: payload.rawCacheChunks,
      rawCacheBytes: payload.rawCacheBytes,
      admissions,
    };
    payload.free();
    self.postMessage(result, {
      transfer: admissions.map((admission) => admission.packedSections.buffer),
    });
  } catch (error: unknown) {
    post({
      type: "error",
      epoch: request.epoch,
      message: errorMessage(error),
    });
  }
};

async function fetchPack(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to load ${url}: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

function post(response: CanonicalWorkerResponse): void {
  self.postMessage(response);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
