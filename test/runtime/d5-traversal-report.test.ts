import { describe, expect, test } from "vitest";
import type { LightingServicePerformanceCounters } from "../../src/runtime/lighting/lighting-protocol";
import {
  evaluateD5Gates,
  summarizeLightingWorker,
  summarizeNumbers,
  type D5DebugStateSnapshot,
  type D5GateThresholds,
  type D5TraversalReportWithoutGates,
} from "../browser/d5-traversal-report";

function lightingCounters(
  overrides: Partial<LightingServicePerformanceCounters> = {},
): LightingServicePerformanceCounters {
  return {
    requestCount: 0,
    requestCountsByType: {},
    maxRequestRoundTripMs: 0,
    pollCount: 0,
    maxResultsPerPoll: 0,
    maxPendingResultsAfterPoll: 0,
    resultCount: 0,
    resultCountsByType: {},
    workerBatchCount: 0,
    maxWorkerBatchSize: 0,
    workerCommandCount: 0,
    workerCommandCountsByType: {},
    maxWorkerCommandDurationMs: 0,
    propagationSliceCount: 0,
    propagationTotalMs: 0,
    maxPropagationSliceMs: 0,
    ...overrides,
  };
}

const PASSING_THRESHOLDS = {
  maxFrameGapMs: 100,
  maxLongTaskCount: 0,
  maxSetChunkViewAckP95Ms: 50,
  maxSetPlayerInputP95Ms: 50,
  maxPollWorldUpdatesP99Ms: 250,
  maxPollWorldUpdatesWithChunksP99Ms: 250,
  maxPlayerSampledTickGapP99Ms: 250,
  maxLightingWorkerCommandDurationMs: 5_000,
  maxLightingWorkerPropagationSliceMs: 100,
  maxPendingVisibleChunkCompileCount: 3_000,
  maxQueuedChunkBuildCount: 3_000,
  maxActiveChunkBuildCount: 16,
  minRenderWorldIngestBatchCount: 1,
  minRenderWorldMeshBuildRequestCount: 1,
  minRenderWorldMeshCompletionCount: 1,
  minMainThreadGpuUploadCount: 1,
} satisfies D5GateThresholds;

function createPassingReport(): D5TraversalReportWithoutGates {
  const emptySummary = summarizeNumbers([]);
  return {
    schemaVersion: 3,
    commit: "test",
    config: {
      seed: "12345",
      preset: "browser_smoke",
      page: "/?mode=debug",
      transport: "remote",
      host: "dedicated_node",
      viewDistance: 8,
      renderDistance: 8,
      fogColor: "#000000",
      startPosition: [0, 80, 0],
      startYawPitch: [0, 0],
      cameraMode: "follow_authoritative_player_state",
      traversalInput: "KeyW+Space",
      targetDurationMs: 1,
      maxDurationMs: 1,
      minChunkBoundaryCrossings: 1,
      sampleIntervalMs: 1,
      viewport: {
        width: 800,
        height: 600,
      },
    },
    environment: {
      commit: "test",
      os: "test",
      arch: "test",
      nodeVersion: "test",
      browserName: "chromium",
      browserVersion: "test",
      userAgent: "test",
      webGpuAdapter: "test",
      devicePixelRatio: 1,
      crossOriginIsolated: false,
      sharedArrayBufferAvailable: false,
    },
    artifacts: {
      startScreenshot: "/tmp/start.png",
      endScreenshot: "/tmp/end.png",
      report: "/tmp/report.json",
      trace: "/tmp/trace.jsonl",
    },
    traversal: {
      durationMs: 1,
      chunkViewChanges: 1,
      chunkPath: [[0, 0]],
      startChunk: [0, 0],
      endChunk: [0, 0],
      finalLoadedChunkCount: 1,
    },
    framePacing: {
      ...summarizeNumbers([16]),
      durationMs: 1,
      frameCount: 2,
      gapCount: 1,
      gapsOver33_4Ms: 0,
      gapsOver50Ms: 0,
      gapsOver100Ms: 0,
      longTaskCount: 0,
      longTaskTotalDurationMs: 0,
      longestLongTaskMs: 0,
    },
    hostResponsiveness: {
      setChunkViewAckLatencyMs: summarizeNumbers([10]),
      setPlayerInputLatencyMs: summarizeNumbers([10]),
      pollWorldUpdatesLatencyMs: summarizeNumbers([10]),
      pollWorldUpdatesWithChunksLatencyMs: summarizeNumbers([10]),
      chunkSnapshotsPerPoll: summarizeNumbers([1]),
      playerTickDelivery: {
        responseMessageCount: 1,
        responseTickGapMs: emptySummary,
        responseTickDelta: emptySummary,
        sampledTickGapMs: summarizeNumbers([50]),
        sampledTickDelta: summarizeNumbers([1]),
        maxSampledWallGapWithoutTickAdvanceMs: 0,
      },
    },
    transport: {
      requestCount: 1,
      failedRequestCount: 0,
      responseBytes: 1,
      byCommand: {},
      requests: [],
    },
    renderWorld: {
      delta: {
        ingestBatchCount: 1,
        meshBuildRequestCount: 1,
        meshNotReadyResponseCount: 0,
        meshCompletionCount: 1,
        mainThreadGpuUploadCount: 1,
      },
    },
    mainThread: {
      maxPendingVisibleChunkCompileCount: 0,
      maxQueuedChunkBuildCount: 0,
      maxActiveChunkBuildCount: 0,
      maxRenderedChunkCount: 1,
    },
    lightingWorker: {
      maxWorkerCommandDurationMs: 10,
      maxPropagationSliceMs: 10,
    },
    samples: [],
    decision: {
      status: "regression_gate",
      nextAction: "test",
      notes: [],
    },
  };
}

describe("D5 traversal report helpers", () => {
  test("summarizes lighting worker counter deltas from debug samples", () => {
    const samples: readonly D5DebugStateSnapshot[] = [
      {
        atMs: 0,
        loadedChunkCount: 0,
        worldPerformance: {
          lighting: lightingCounters({
            requestCount: 1,
            requestCountsByType: { request_initial_light: 1 },
            workerBatchCount: 1,
            maxWorkerBatchSize: 1,
            workerCommandCount: 1,
            workerCommandCountsByType: { request_initial_light: 1 },
            maxWorkerCommandDurationMs: 12,
            propagationSliceCount: 2,
            propagationTotalMs: 8,
            maxPropagationSliceMs: 5,
          }),
        },
      },
      {
        atMs: 10,
        loadedChunkCount: 1,
        worldPerformance: {
          lighting: lightingCounters({
            requestCount: 4,
            requestCountsByType: { request_initial_light: 3, upsert_light_chunk: 1 },
            workerBatchCount: 2,
            maxWorkerBatchSize: 2,
            workerCommandCount: 4,
            workerCommandCountsByType: { request_initial_light: 3, upsert_light_chunk: 1 },
            maxWorkerCommandDurationMs: 30,
            propagationSliceCount: 7,
            propagationTotalMs: 26,
            maxPropagationSliceMs: 9,
          }),
        },
      },
    ];

    const summary = summarizeLightingWorker(samples);

    expect(summary.delta).toMatchObject({
      requestCount: 3,
      requestCountsByType: {
        request_initial_light: 2,
        upsert_light_chunk: 1,
      },
      workerBatchCount: 1,
      maxWorkerBatchSize: 2,
      workerCommandCount: 3,
      propagationSliceCount: 5,
      propagationTotalMs: 18,
    });
    expect(summary.maxWorkerCommandDurationMs).toBe(30);
    expect(summary.maxPropagationSliceMs).toBe(9);
  });

  test("fails the traversal gate when lighting or GPU publish counters cross thresholds", () => {
    const passingReport = createPassingReport();
    expect(evaluateD5Gates(passingReport, PASSING_THRESHOLDS).passed).toBe(true);

    const failingReport = {
      ...passingReport,
      renderWorld: {
        delta: {
          ingestBatchCount: 0,
          meshBuildRequestCount: 0,
          meshNotReadyResponseCount: 0,
          meshCompletionCount: 0,
          mainThreadGpuUploadCount: 0,
        },
      },
      lightingWorker: {
        ...passingReport.lightingWorker,
        maxPropagationSliceMs: 150,
      },
    } satisfies D5TraversalReportWithoutGates;

    const gates = evaluateD5Gates(failingReport, PASSING_THRESHOLDS);

    expect(gates.passed).toBe(false);
    expect(gates.failures).toEqual(expect.arrayContaining([
      expect.stringContaining("lighting.propagation_slice_max_ms"),
      expect.stringContaining("render_world.ingest_batches"),
      expect.stringContaining("render_world.mesh_build_requests"),
      expect.stringContaining("render_world.mesh_completions"),
      expect.stringContaining("main_thread.gpu_uploads"),
    ]));
  });
});
