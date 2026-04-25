import type { LightingServicePerformanceCounters } from "../../src/runtime/lighting/lighting-protocol";
import type { WorldPerformanceSnapshot } from "../../src/runtime/protocol/world-messages";

export const D5_TRAVERSAL_REPORT_SCHEMA_VERSION = 3;

export type D5TransportCommand =
  | "open_world"
  | "set_chunk_view"
  | "set_player_input"
  | "poll_world_updates";

export interface D5TraversalConfig {
  readonly seed: string;
  readonly preset: "browser_smoke";
  readonly page: "/debug.html";
  readonly transport: "remote";
  readonly host: "dedicated_node";
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly fogColor: string;
  readonly startPosition: readonly [number, number, number];
  readonly startYawPitch: readonly [number, number];
  readonly cameraMode: "follow_authoritative_player_state";
  readonly traversalInput: "KeyW+Space";
  readonly targetDurationMs: number;
  readonly maxDurationMs: number;
  readonly minChunkBoundaryCrossings: number;
  readonly sampleIntervalMs: number;
  readonly viewport: {
    readonly width: number;
    readonly height: number;
  };
}

export interface D5Environment {
  readonly commit: string;
  readonly os: string;
  readonly arch: string;
  readonly nodeVersion: string;
  readonly browserName: string;
  readonly browserVersion: string;
  readonly userAgent: string;
  readonly webGpuAdapter: string;
  readonly devicePixelRatio: number;
  readonly crossOriginIsolated: boolean;
  readonly sharedArrayBufferAvailable: boolean;
}

export interface D5Artifacts {
  readonly startScreenshot: string;
  readonly endScreenshot: string;
  readonly report: string;
  readonly trace: string;
}

export interface D5RenderWorldCounters {
  readonly ingestBatchCount: number;
  readonly meshBuildRequestCount: number;
  readonly meshNotReadyResponseCount: number;
  readonly meshCompletionCount: number;
  readonly mainThreadGpuUploadCount: number;
}

export interface D5RenderQueueStats {
  readonly renderedChunkCount: number;
  readonly pendingVisibleChunkCompileCount: number;
  readonly queuedChunkBuildCount: number;
  readonly activeChunkBuildCount: number;
}

export interface D5DebugStateSnapshot {
  readonly atMs: number;
  readonly playerTick?: number;
  readonly playerPosition?: readonly [number, number, number];
  readonly playerChunk?: readonly [number, number];
  readonly chunkViewCenter?: readonly [number, number];
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount?: number;
  readonly renderWorldCounters?: D5RenderWorldCounters;
  readonly renderQueueStats?: D5RenderQueueStats;
  readonly worldPerformance?: WorldPerformanceSnapshot;
}

export interface D5LongTaskRecord {
  readonly startTimeMs: number;
  readonly durationMs: number;
  readonly nearestMarker?: string;
}

export interface D5FrameProbeResult {
  readonly timeOriginMs: number;
  readonly startTimeMs: number;
  readonly endTimeMs: number;
  readonly frameTimesMs: readonly number[];
  readonly longTasks: readonly D5LongTaskRecord[];
  readonly markers: readonly D5PhaseMarker[];
}

export interface D5PhaseMarker {
  readonly name: string;
  readonly atMs: number;
}

export interface D5NumericSummary {
  readonly count: number;
  readonly min: number;
  readonly p50: number;
  readonly p95: number;
  readonly p99: number;
  readonly max: number;
}

export interface D5FramePacingSummary extends D5NumericSummary {
  readonly durationMs: number;
  readonly frameCount: number;
  readonly gapCount: number;
  readonly gapsOver33_4Ms: number;
  readonly gapsOver50Ms: number;
  readonly gapsOver100Ms: number;
  readonly longTaskCount: number;
  readonly longTaskTotalDurationMs: number;
  readonly longestLongTaskMs: number;
  readonly longestLongTaskNearestMarker?: string;
}

export interface D5TraversalSummary {
  readonly durationMs: number;
  readonly chunkViewChanges: number;
  readonly chunkPath: readonly (readonly [number, number])[];
  readonly startChunk: readonly [number, number];
  readonly endChunk: readonly [number, number];
  readonly startPosition?: readonly [number, number, number];
  readonly endPosition?: readonly [number, number, number];
  readonly finalLoadedChunkCount: number;
  readonly expectedLoadedChunkCount?: number;
}

export interface D5TransportRequestRecord {
  readonly command: D5TransportCommand;
  readonly method: string;
  readonly url: string;
  readonly status?: number;
  readonly startMs: number;
  readonly endMs?: number;
  readonly durationMs?: number;
  readonly requestBytes: number;
  readonly responseBytes?: number;
  readonly responseMessageTypes?: readonly string[];
  readonly responseMessageCount?: number;
  readonly chunkSnapshotCount?: number;
  readonly playerStateTicks?: readonly number[];
  readonly error?: string;
}

export interface D5TransportCommandSummary {
  readonly requestCount: number;
  readonly failedRequestCount: number;
  readonly requestDurationMs: D5NumericSummary;
  readonly requestBytes: number;
  readonly responseBytes: number;
  readonly responseMessageCount: number;
  readonly chunkSnapshotCount: number;
  readonly playerStateMessageCount: number;
}

export interface D5TransportSummary {
  readonly requestCount: number;
  readonly failedRequestCount: number;
  readonly responseBytes: number;
  readonly byCommand: Partial<Record<D5TransportCommand, D5TransportCommandSummary>>;
  readonly requests: readonly D5TransportRequestRecord[];
}

export interface D5PlayerTickDeliverySummary {
  readonly responseMessageCount: number;
  readonly responseTickGapMs: D5NumericSummary;
  readonly responseTickDelta: D5NumericSummary;
  readonly sampledTickGapMs: D5NumericSummary;
  readonly sampledTickDelta: D5NumericSummary;
  readonly maxSampledWallGapWithoutTickAdvanceMs: number;
}

export interface D5HostResponsivenessSummary {
  readonly setChunkViewAckLatencyMs: D5NumericSummary;
  readonly setPlayerInputLatencyMs: D5NumericSummary;
  readonly pollWorldUpdatesLatencyMs: D5NumericSummary;
  readonly pollWorldUpdatesWithChunksLatencyMs: D5NumericSummary;
  readonly chunkSnapshotsPerPoll: D5NumericSummary;
  readonly playerTickDelivery: D5PlayerTickDeliverySummary;
}

export interface D5RenderWorldSummary {
  readonly start?: D5RenderWorldCounters;
  readonly end?: D5RenderWorldCounters;
  readonly delta?: D5RenderWorldCounters;
}

export interface D5MainThreadSummary {
  readonly startQueue?: D5RenderQueueStats;
  readonly endQueue?: D5RenderQueueStats;
  readonly maxPendingVisibleChunkCompileCount: number;
  readonly maxQueuedChunkBuildCount: number;
  readonly maxActiveChunkBuildCount: number;
  readonly maxRenderedChunkCount: number;
}

export interface D5LightingWorkerSummary {
  readonly start?: LightingServicePerformanceCounters;
  readonly end?: LightingServicePerformanceCounters;
  readonly delta?: LightingServicePerformanceCounters;
  readonly maxWorkerCommandDurationMs: number;
  readonly maxPropagationSliceMs: number;
}

export interface D5GateThresholds {
  readonly maxFrameGapMs: number;
  readonly maxLongTaskCount: number;
  readonly maxSetChunkViewAckP95Ms: number;
  readonly maxSetPlayerInputP95Ms: number;
  readonly maxPollWorldUpdatesP99Ms: number;
  readonly maxPollWorldUpdatesWithChunksP99Ms: number;
  readonly maxPlayerSampledTickGapP99Ms: number;
  readonly maxLightingWorkerCommandDurationMs: number;
  readonly maxLightingWorkerPropagationSliceMs: number;
  readonly maxPendingVisibleChunkCompileCount: number;
  readonly maxQueuedChunkBuildCount: number;
  readonly maxActiveChunkBuildCount: number;
  readonly minRenderWorldIngestBatchCount: number;
  readonly minRenderWorldMeshBuildRequestCount: number;
  readonly minRenderWorldMeshCompletionCount: number;
  readonly minMainThreadGpuUploadCount: number;
}

export interface D5GateResult {
  readonly name: string;
  readonly status: "pass" | "fail";
  readonly value: number;
  readonly threshold: number;
  readonly comparator: "<=" | ">=";
  readonly details?: string;
}

export interface D5GateSummary {
  readonly passed: boolean;
  readonly thresholds: D5GateThresholds;
  readonly results: readonly D5GateResult[];
  readonly failures: readonly string[];
}

export interface D5Decision {
  readonly status: "first_slice_only" | "regression_gate";
  readonly nextAction: string;
  readonly notes: readonly string[];
}

export interface D5TraversalReport {
  readonly schemaVersion: typeof D5_TRAVERSAL_REPORT_SCHEMA_VERSION;
  readonly commit: string;
  readonly config: D5TraversalConfig;
  readonly environment: D5Environment;
  readonly artifacts: D5Artifacts;
  readonly traversal: D5TraversalSummary;
  readonly framePacing: D5FramePacingSummary;
  readonly hostResponsiveness: D5HostResponsivenessSummary;
  readonly transport: D5TransportSummary;
  readonly renderWorld: D5RenderWorldSummary;
  readonly mainThread: D5MainThreadSummary;
  readonly lightingWorker: D5LightingWorkerSummary;
  readonly gates: D5GateSummary;
  readonly samples: readonly D5DebugStateSnapshot[];
  readonly decision: D5Decision;
}

export type D5TraversalReportWithoutGates = Omit<D5TraversalReport, "gates">;

function percentile(sortedValues: readonly number[], percentileValue: number): number {
  if (sortedValues.length === 0) {
    return 0;
  }

  const index = Math.min(
    sortedValues.length - 1,
    Math.max(0, Math.ceil((percentileValue / 100) * sortedValues.length) - 1),
  );
  return sortedValues[index]!;
}

export function summarizeNumbers(values: readonly number[]): D5NumericSummary {
  if (values.length === 0) {
    return {
      count: 0,
      min: 0,
      p50: 0,
      p95: 0,
      p99: 0,
      max: 0,
    };
  }

  const sortedValues = [...values].sort((left, right) => left - right);
  return {
    count: values.length,
    min: sortedValues[0]!,
    p50: percentile(sortedValues, 50),
    p95: percentile(sortedValues, 95),
    p99: percentile(sortedValues, 99),
    max: sortedValues[sortedValues.length - 1]!,
  };
}

export function summarizeFramePacing(
  probe: D5FrameProbeResult,
  traversalDurationMs: number,
): D5FramePacingSummary {
  const gaps: number[] = [];
  for (let index = 1; index < probe.frameTimesMs.length; index++) {
    gaps.push(probe.frameTimesMs[index]! - probe.frameTimesMs[index - 1]!);
  }

  const gapSummary = summarizeNumbers(gaps);
  const longestLongTask = probe.longTasks.reduce<D5LongTaskRecord | undefined>((longest, task) => {
    if (longest === undefined || task.durationMs > longest.durationMs) {
      return task;
    }

    return longest;
  }, undefined);

  return {
    ...gapSummary,
    durationMs: traversalDurationMs,
    frameCount: probe.frameTimesMs.length,
    gapCount: gaps.length,
    gapsOver33_4Ms: gaps.filter((gap) => gap > 33.4).length,
    gapsOver50Ms: gaps.filter((gap) => gap > 50).length,
    gapsOver100Ms: gaps.filter((gap) => gap > 100).length,
    longTaskCount: probe.longTasks.length,
    longTaskTotalDurationMs: probe.longTasks.reduce((total, task) => total + task.durationMs, 0),
    longestLongTaskMs: longestLongTask?.durationMs ?? 0,
    longestLongTaskNearestMarker: longestLongTask?.nearestMarker,
  };
}

export function diffRenderWorldCounters(
  start: D5RenderWorldCounters | undefined,
  end: D5RenderWorldCounters | undefined,
): D5RenderWorldCounters | undefined {
  if (start === undefined || end === undefined) {
    return undefined;
  }

  return {
    ingestBatchCount: end.ingestBatchCount - start.ingestBatchCount,
    meshBuildRequestCount: end.meshBuildRequestCount - start.meshBuildRequestCount,
    meshNotReadyResponseCount: end.meshNotReadyResponseCount - start.meshNotReadyResponseCount,
    meshCompletionCount: end.meshCompletionCount - start.meshCompletionCount,
    mainThreadGpuUploadCount: end.mainThreadGpuUploadCount - start.mainThreadGpuUploadCount,
  };
}

function diffCountsByType(
  start: Readonly<Record<string, number>>,
  end: Readonly<Record<string, number>>,
): Readonly<Record<string, number>> {
  const keys = new Set([...Object.keys(start), ...Object.keys(end)]);
  const diff: Record<string, number> = {};
  for (const key of keys) {
    const value = (end[key] ?? 0) - (start[key] ?? 0);
    if (value !== 0) {
      diff[key] = value;
    }
  }

  return diff;
}

export function diffLightingServicePerformanceCounters(
  start: LightingServicePerformanceCounters | undefined,
  end: LightingServicePerformanceCounters | undefined,
): LightingServicePerformanceCounters | undefined {
  if (start === undefined || end === undefined) {
    return undefined;
  }

  return {
    requestCount: end.requestCount - start.requestCount,
    requestCountsByType: diffCountsByType(start.requestCountsByType, end.requestCountsByType),
    maxRequestRoundTripMs: end.maxRequestRoundTripMs,
    pollCount: end.pollCount - start.pollCount,
    maxResultsPerPoll: end.maxResultsPerPoll,
    maxPendingResultsAfterPoll: end.maxPendingResultsAfterPoll,
    resultCount: end.resultCount - start.resultCount,
    resultCountsByType: diffCountsByType(start.resultCountsByType, end.resultCountsByType),
    workerCommandCount: end.workerCommandCount - start.workerCommandCount,
    workerCommandCountsByType: diffCountsByType(start.workerCommandCountsByType, end.workerCommandCountsByType),
    maxWorkerCommandDurationMs: end.maxWorkerCommandDurationMs,
    propagationSliceCount: end.propagationSliceCount - start.propagationSliceCount,
    propagationTotalMs: end.propagationTotalMs - start.propagationTotalMs,
    maxPropagationSliceMs: end.maxPropagationSliceMs,
  };
}

export function summarizeLightingWorker(
  samples: readonly D5DebugStateSnapshot[],
): D5LightingWorkerSummary {
  const lightingSamples = samples.flatMap((sample) => (
    sample.worldPerformance?.lighting === undefined ? [] : [sample.worldPerformance.lighting]
  ));
  const start = lightingSamples[0];
  const end = lightingSamples[lightingSamples.length - 1];
  const delta = diffLightingServicePerformanceCounters(start, end);

  return {
    start,
    end,
    delta,
    maxWorkerCommandDurationMs: Math.max(0, ...lightingSamples.map((sample) => sample.maxWorkerCommandDurationMs)),
    maxPropagationSliceMs: Math.max(0, ...lightingSamples.map((sample) => sample.maxPropagationSliceMs)),
  };
}

function requestDurations(
  requests: readonly D5TransportRequestRecord[],
  command: D5TransportCommand,
  predicate: (request: D5TransportRequestRecord) => boolean = () => true,
): number[] {
  return requests.flatMap((request) => (
    request.command === command && request.durationMs !== undefined && predicate(request)
      ? [request.durationMs]
      : []
  ));
}

export function summarizeHostResponsiveness(
  requests: readonly D5TransportRequestRecord[],
  samples: readonly D5DebugStateSnapshot[],
): D5HostResponsivenessSummary {
  const tickResponses = requests
    .flatMap((request) => (request.endMs === undefined
      ? []
      : (request.playerStateTicks ?? []).map((tick) => ({ atMs: request.endMs!, tick }))))
    .sort((left, right) => left.atMs - right.atMs);
  const responseTickGaps: number[] = [];
  const responseTickDeltas: number[] = [];
  for (let index = 1; index < tickResponses.length; index++) {
    const previous = tickResponses[index - 1]!;
    const next = tickResponses[index]!;
    if (next.tick <= previous.tick) {
      continue;
    }

    responseTickGaps.push(next.atMs - previous.atMs);
    responseTickDeltas.push(next.tick - previous.tick);
  }

  const sampledTicks = samples
    .flatMap((sample) => sample.playerTick === undefined ? [] : [{ atMs: sample.atMs, tick: sample.playerTick }])
    .sort((left, right) => left.atMs - right.atMs);
  const sampledTickGaps: number[] = [];
  const sampledTickDeltas: number[] = [];
  let maxSampledWallGapWithoutTickAdvanceMs = 0;
  let lastAdvancedSample = sampledTicks[0];
  for (let index = 1; index < sampledTicks.length; index++) {
    const previous = sampledTicks[index - 1]!;
    const next = sampledTicks[index]!;
    if (next.tick > previous.tick) {
      if (lastAdvancedSample !== undefined) {
        sampledTickGaps.push(next.atMs - lastAdvancedSample.atMs);
        sampledTickDeltas.push(next.tick - lastAdvancedSample.tick);
      }
      lastAdvancedSample = next;
      continue;
    }

    if (lastAdvancedSample !== undefined) {
      maxSampledWallGapWithoutTickAdvanceMs = Math.max(
        maxSampledWallGapWithoutTickAdvanceMs,
        next.atMs - lastAdvancedSample.atMs,
      );
    }
  }

  const pollChunkSnapshotCounts = requests.flatMap((request) => (
    request.command === "poll_world_updates" && (request.chunkSnapshotCount ?? 0) > 0
      ? [request.chunkSnapshotCount!]
      : []
  ));

  return {
    setChunkViewAckLatencyMs: summarizeNumbers(requestDurations(requests, "set_chunk_view")),
    setPlayerInputLatencyMs: summarizeNumbers(requestDurations(requests, "set_player_input")),
    pollWorldUpdatesLatencyMs: summarizeNumbers(requestDurations(requests, "poll_world_updates")),
    pollWorldUpdatesWithChunksLatencyMs: summarizeNumbers(requestDurations(
      requests,
      "poll_world_updates",
      (request) => (request.chunkSnapshotCount ?? 0) > 0,
    )),
    chunkSnapshotsPerPoll: summarizeNumbers(pollChunkSnapshotCounts),
    playerTickDelivery: {
      responseMessageCount: tickResponses.length,
      responseTickGapMs: summarizeNumbers(responseTickGaps),
      responseTickDelta: summarizeNumbers(responseTickDeltas),
      sampledTickGapMs: summarizeNumbers(sampledTickGaps),
      sampledTickDelta: summarizeNumbers(sampledTickDeltas),
      maxSampledWallGapWithoutTickAdvanceMs,
    },
  };
}

function gate(
  name: string,
  value: number,
  threshold: number,
  comparator: "<=" | ">=",
  details?: string,
): D5GateResult {
  const passed = comparator === "<=" ? value <= threshold : value >= threshold;
  return {
    name,
    status: passed ? "pass" : "fail",
    value,
    threshold,
    comparator,
    details,
  };
}

export function evaluateD5Gates(
  report: D5TraversalReportWithoutGates,
  thresholds: D5GateThresholds,
): D5GateSummary {
  const results: D5GateResult[] = [
    gate("frame.max_gap_ms", report.framePacing.max, thresholds.maxFrameGapMs, "<="),
    gate("frame.long_task_count", report.framePacing.longTaskCount, thresholds.maxLongTaskCount, "<="),
    gate("host.set_chunk_view_ack_p95_ms", report.hostResponsiveness.setChunkViewAckLatencyMs.p95, thresholds.maxSetChunkViewAckP95Ms, "<="),
    gate("host.set_player_input_p95_ms", report.hostResponsiveness.setPlayerInputLatencyMs.p95, thresholds.maxSetPlayerInputP95Ms, "<="),
    gate("host.poll_world_updates_p99_ms", report.hostResponsiveness.pollWorldUpdatesLatencyMs.p99, thresholds.maxPollWorldUpdatesP99Ms, "<="),
    gate(
      "host.poll_world_updates_with_chunks_p99_ms",
      report.hostResponsiveness.pollWorldUpdatesWithChunksLatencyMs.p99,
      thresholds.maxPollWorldUpdatesWithChunksP99Ms,
      "<=",
    ),
    gate(
      "host.player_sampled_tick_gap_p99_ms",
      report.hostResponsiveness.playerTickDelivery.sampledTickGapMs.p99,
      thresholds.maxPlayerSampledTickGapP99Ms,
      "<=",
    ),
    gate(
      "lighting.worker_command_max_ms",
      report.lightingWorker.maxWorkerCommandDurationMs,
      thresholds.maxLightingWorkerCommandDurationMs,
      "<=",
      "largest worker-side lighting command wall time observed through light_performance",
    ),
    gate(
      "lighting.propagation_slice_max_ms",
      report.lightingWorker.maxPropagationSliceMs,
      thresholds.maxLightingWorkerPropagationSliceMs,
      "<=",
      "largest LevelLightEngine.runUpdates slice observed inside the lighting worker",
    ),
    gate(
      "render_world.ingest_batches",
      report.renderWorld.delta?.ingestBatchCount ?? 0,
      thresholds.minRenderWorldIngestBatchCount,
      ">=",
    ),
    gate(
      "render_world.mesh_build_requests",
      report.renderWorld.delta?.meshBuildRequestCount ?? 0,
      thresholds.minRenderWorldMeshBuildRequestCount,
      ">=",
    ),
    gate(
      "render_world.mesh_completions",
      report.renderWorld.delta?.meshCompletionCount ?? 0,
      thresholds.minRenderWorldMeshCompletionCount,
      ">=",
    ),
    gate(
      "main_thread.gpu_uploads",
      report.renderWorld.delta?.mainThreadGpuUploadCount ?? 0,
      thresholds.minMainThreadGpuUploadCount,
      ">=",
    ),
    gate(
      "main_thread.pending_visible_chunk_compiles",
      report.mainThread.maxPendingVisibleChunkCompileCount,
      thresholds.maxPendingVisibleChunkCompileCount,
      "<=",
    ),
    gate(
      "main_thread.queued_chunk_builds",
      report.mainThread.maxQueuedChunkBuildCount,
      thresholds.maxQueuedChunkBuildCount,
      "<=",
    ),
    gate(
      "main_thread.active_chunk_builds",
      report.mainThread.maxActiveChunkBuildCount,
      thresholds.maxActiveChunkBuildCount,
      "<=",
    ),
  ];
  const failures = results
    .filter((result) => result.status === "fail")
    .map((result) => `${result.name}: ${result.value.toFixed(1)} ${result.comparator} ${result.threshold.toFixed(1)}`);

  return {
    passed: failures.length === 0,
    thresholds,
    results,
    failures,
  };
}
