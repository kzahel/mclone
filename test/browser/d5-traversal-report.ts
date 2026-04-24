export const D5_TRAVERSAL_REPORT_SCHEMA_VERSION = 1;

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
  readonly error?: string;
}

export interface D5TransportCommandSummary {
  readonly requestCount: number;
  readonly failedRequestCount: number;
  readonly requestDurationMs: D5NumericSummary;
  readonly requestBytes: number;
  readonly responseBytes: number;
}

export interface D5TransportSummary {
  readonly requestCount: number;
  readonly failedRequestCount: number;
  readonly responseBytes: number;
  readonly byCommand: Partial<Record<D5TransportCommand, D5TransportCommandSummary>>;
  readonly requests: readonly D5TransportRequestRecord[];
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

export interface D5Decision {
  readonly status: "first_slice_only";
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
  readonly transport: D5TransportSummary;
  readonly renderWorld: D5RenderWorldSummary;
  readonly mainThread: D5MainThreadSummary;
  readonly samples: readonly D5DebugStateSnapshot[];
  readonly decision: D5Decision;
}

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
