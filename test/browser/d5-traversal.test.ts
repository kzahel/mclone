import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { arch, platform, release } from "node:os";
import process from "node:process";
import { performance } from "node:perf_hooks";
import { expect, test, type Page } from "./remote-world-host-fixture";
import {
  D5_TRAVERSAL_REPORT_SCHEMA_VERSION,
  diffRenderWorldCounters,
  evaluateD5Gates,
  summarizeFramePacing,
  summarizeHostResponsiveness,
  summarizeLightingWorker,
  summarizeNumbers,
  type D5Artifacts,
  type D5DebugStateSnapshot,
  type D5Environment,
  type D5FrameProbeResult,
  type D5PhaseMarker,
  type D5RenderQueueStats,
  type D5RenderWorldCounters,
  type D5TransportCommand,
  type D5TransportRequestRecord,
  type D5TransportSummary,
  type D5TraversalConfig,
  type D5TraversalReport,
  type D5TraversalReportWithoutGates,
} from "./d5-traversal-report";

const START_SCREENSHOT_PATH = "/tmp/mclone-d5-start.png";
const END_SCREENSHOT_PATH = "/tmp/mclone-d5-end.png";
const REPORT_PATH = "/tmp/mclone-d5-traversal-report.json";
const TRACE_PATH = "/tmp/mclone-d5-traversal-trace.json";
const EXPECTED_LOADED_CHUNK_COUNT = 225;

const D5_CONFIG = {
  seed: "12345",
  preset: "browser_smoke",
  page: "/debug.html",
  transport: "remote",
  host: "dedicated_node",
  viewDistance: 6,
  renderDistance: 192,
  fogColor: "8fb8ff",
  startPosition: [965.5, 168, 3189.5],
  startYawPitch: [225, 55],
  cameraMode: "follow_authoritative_player_state",
  traversalInput: "KeyW+Space",
  targetDurationMs: 30_000,
  maxDurationMs: 45_000,
  minChunkBoundaryCrossings: 4,
  sampleIntervalMs: 250,
  viewport: {
    width: 1280,
    height: 720,
  },
} as const satisfies D5TraversalConfig;

const D5_GATE_THRESHOLDS = {
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
  minMainThreadGpuUploadCount: 1,
} as const;

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerTick?: number;
  readonly playerPosition?: readonly [number, number, number];
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly chunkViewCenterX?: number;
  readonly chunkViewCenterZ?: number;
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount?: number;
  readonly viewDistance?: number;
  readonly renderDistance?: number;
  readonly frameCount: number;
  readonly renderWorldCounters?: D5RenderWorldCounters;
  readonly renderQueueStats?: D5RenderQueueStats;
  readonly worldPerformance?: D5DebugStateSnapshot["worldPerformance"];
  readonly error?: string;
}

interface DebugInjectedInput {
  readonly locked?: boolean;
  readonly heldKeys?: readonly string[];
}

interface DebugRuntimeController {
  readonly state: DebugRuntimeState;
  setInjectedInput(input: DebugInjectedInput | null): void;
}

interface D5FrameProbe {
  mark(name: string): void;
  stop(): D5FrameProbeResult;
}

type DebugWindow = Window & {
  __mcloneDebug?: DebugRuntimeController;
  __mcloneD5FrameProbe?: D5FrameProbe;
  __mcloneD5FetchRecords?: MutableNetworkRecord[];
  __mcloneD5FetchRecorderFlush?: () => Promise<void>;
};

interface MutableNetworkRecord {
  command: D5TransportCommand;
  method: string;
  url: string;
  startMs: number;
  requestBytes: number;
  status?: number;
  endMs?: number;
  responseBytes?: number;
  responseMessageTypes?: string[];
  responseMessageCount?: number;
  chunkSnapshotCount?: number;
  playerStateTicks?: number[];
  error?: string;
}

interface TraceEvent {
  readonly name: string;
  readonly ph: "i" | "X";
  readonly ts: number;
  readonly dur?: number;
  readonly pid: number;
  readonly tid: number;
  readonly s?: "t";
  readonly args?: Record<string, unknown>;
}

test.skip(process.env.MCLONE_RUN_D5 !== "1", "run with pnpm perf:d5");
test.setTimeout(420_000);

const DEBUG_READY_TIMEOUT_MS = 180_000;
const SETTLED_FRAME_TIMEOUT_MS = 180_000;

function createDebugUrl(remoteWorldHostUrl: string): string {
  return `/debug.html?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
    cameraX: D5_CONFIG.startPosition[0].toString(),
    cameraY: D5_CONFIG.startPosition[1].toString(),
    cameraZ: D5_CONFIG.startPosition[2].toString(),
    cameraYaw: D5_CONFIG.startYawPitch[0].toString(),
    cameraPitch: D5_CONFIG.startYawPitch[1].toString(),
    viewDistance: D5_CONFIG.viewDistance.toString(),
    renderDistance: D5_CONFIG.renderDistance.toString(),
    fogColor: D5_CONFIG.fogColor,
  }).toString()}`;
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

function isWebGpuConsoleError(message: string): boolean {
  return /\b(WebGPU|GPUValidationError|validation)\b/i.test(message);
}

function readGitCommit(): string {
  try {
    return execFileSync("git", ["rev-parse", "--short=12", "HEAD"], {
      cwd: process.cwd(),
      encoding: "utf8",
    }).trim();
  } catch {
    return "unknown";
  }
}

function chunkPair(state: DebugRuntimeState): readonly [number, number] | undefined {
  if (state.playerChunkX === undefined || state.playerChunkZ === undefined) {
    return undefined;
  }

  return [state.playerChunkX, state.playerChunkZ];
}

function sameChunkPair(left: readonly [number, number] | undefined, right: readonly [number, number]): boolean {
  return left !== undefined && left[0] === right[0] && left[1] === right[1];
}

function createSnapshot(atMs: number, state: DebugRuntimeState): D5DebugStateSnapshot {
  return {
    atMs,
    playerTick: state.playerTick,
    playerPosition: state.playerPosition,
    playerChunk: chunkPair(state),
    chunkViewCenter: state.chunkViewCenterX === undefined || state.chunkViewCenterZ === undefined
      ? undefined
      : [state.chunkViewCenterX, state.chunkViewCenterZ],
    loadedChunkCount: state.loadedChunkCount,
    expectedLoadedChunkCount: state.expectedLoadedChunkCount,
    renderWorldCounters: state.renderWorldCounters,
    renderQueueStats: state.renderQueueStats,
    worldPerformance: state.worldPerformance,
  };
}

function isRenderQueueSettled(state: DebugRuntimeState): boolean {
  const stats = state.renderQueueStats;
  return stats !== undefined
    && stats.renderedChunkCount > 0
    && stats.pendingVisibleChunkCompileCount === 0
    && stats.queuedChunkBuildCount === 0
    && stats.activeChunkBuildCount === 0;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => {
    const debug = (window as DebugWindow).__mcloneDebug;
    if (debug === undefined) {
      throw new Error("window.__mcloneDebug is not installed");
    }

    return debug.state;
  });
}

async function waitForDebugReady(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as DebugWindow).__mcloneDebug !== undefined, undefined, { timeout: DEBUG_READY_TIMEOUT_MS });
  await page.waitForFunction(() => {
    const state = (window as DebugWindow).__mcloneDebug?.state;
    return state?.ready === true || state?.error !== undefined;
  }, undefined, { timeout: DEBUG_READY_TIMEOUT_MS });
}

async function waitForLoadedSettledFrame(page: Page, expectedLoadedChunkCount: number): Promise<DebugRuntimeState> {
  await page.waitForFunction((expected) => {
    const state = (window as DebugWindow).__mcloneDebug?.state;
    if (state === undefined || state.error !== undefined) {
      return true;
    }

    const stats = state.renderQueueStats;
    return state.loadedChunkCount === expected
      && stats !== undefined
      && stats.renderedChunkCount > 0
      && stats.pendingVisibleChunkCompileCount === 0
      && stats.queuedChunkBuildCount === 0
      && stats.activeChunkBuildCount === 0;
  }, expectedLoadedChunkCount, { timeout: SETTLED_FRAME_TIMEOUT_MS });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.loadedChunkCount).toBe(expectedLoadedChunkCount);
  expect(isRenderQueueSettled(state)).toBe(true);
  return state;
}

async function setTraversalInput(page: Page, enabled: boolean): Promise<void> {
  await page.evaluate((isEnabled) => {
    const debug = (window as DebugWindow).__mcloneDebug;
    if (debug === undefined) {
      throw new Error("window.__mcloneDebug is not installed");
    }

    debug.setInjectedInput(isEnabled
      ? {
          locked: true,
          heldKeys: ["KeyW", "Space"],
        }
      : null);
  }, enabled);
}

async function startFrameProbe(page: Page): Promise<void> {
  await page.evaluate(() => {
    const d5Window = window as DebugWindow;
    const frameTimesMs: number[] = [];
    const longTasks: D5FrameProbeResult["longTasks"][number][] = [];
    const markers: D5PhaseMarker[] = [];
    const startTimeMs = performance.now();
    let rafId = 0;
    let running = true;

    const findNearestMarker = (startTime: number): string | undefined => {
      let nearest: D5PhaseMarker | undefined;
      for (const marker of markers) {
        if (marker.atMs <= startTime && (nearest === undefined || marker.atMs > nearest.atMs)) {
          nearest = marker;
        }
      }

      return nearest?.name;
    };

    const observer = typeof PerformanceObserver === "undefined"
      || !PerformanceObserver.supportedEntryTypes.includes("longtask")
      ? undefined
      : new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            longTasks.push({
              startTimeMs: entry.startTime,
              durationMs: entry.duration,
              nearestMarker: findNearestMarker(entry.startTime),
            });
          }
        });
    observer?.observe({ entryTypes: ["longtask"] });

    const tick = (timestamp: number): void => {
      if (!running) {
        return;
      }

      frameTimesMs.push(timestamp);
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);

    d5Window.__mcloneD5FrameProbe = {
      mark(name: string): void {
        markers.push({ name, atMs: performance.now() });
      },
      stop(): D5FrameProbeResult {
        running = false;
        cancelAnimationFrame(rafId);
        observer?.disconnect();
        return {
          timeOriginMs: performance.timeOrigin,
          startTimeMs,
          endTimeMs: performance.now(),
          frameTimesMs,
          longTasks,
          markers,
        };
      },
    };
  });
}

async function markFrameProbe(page: Page, name: string): Promise<void> {
  await page.evaluate((markerName) => {
    const probe = (window as DebugWindow).__mcloneD5FrameProbe;
    if (probe === undefined) {
      throw new Error("window.__mcloneD5FrameProbe is not installed");
    }

    probe.mark(markerName);
  }, name);
}

async function stopFrameProbe(page: Page): Promise<D5FrameProbeResult> {
  return await page.evaluate(() => {
    const d5Window = window as DebugWindow;
    const probe = d5Window.__mcloneD5FrameProbe;
    if (probe === undefined) {
      throw new Error("window.__mcloneD5FrameProbe is not installed");
    }

    const result = probe.stop();
    d5Window.__mcloneD5FrameProbe = undefined;
    return result;
  });
}

async function installBrowserFetchRecorder(page: Page): Promise<void> {
  await page.addInitScript(() => {
    type BrowserRecord = MutableNetworkRecord;
    const d5Window = window as DebugWindow;
    const records: BrowserRecord[] = [];
    const pendingBodies: Promise<void>[] = [];
    const originalFetch = window.fetch.bind(window);

    const classify = (urlValue: string): D5TransportCommand | undefined => {
      const url = new URL(urlValue, window.location.href);
      if (url.pathname === "/api/world/session") {
        return "open_world";
      }

      if (/^\/api\/world\/session\/[^/]+\/chunk-view$/.test(url.pathname)) {
        return "set_chunk_view";
      }

      if (/^\/api\/world\/session\/[^/]+\/player-input$/.test(url.pathname)) {
        return "set_player_input";
      }

      if (/^\/api\/world\/session\/[^/]+\/updates\/poll$/.test(url.pathname)) {
        return "poll_world_updates";
      }

      return undefined;
    };

    const byteLength = (value: string): number => new TextEncoder().encode(value).byteLength;
    const extractMetrics = (body: string): Pick<
      BrowserRecord,
      "responseMessageTypes" | "responseMessageCount" | "chunkSnapshotCount" | "playerStateTicks"
    > => {
      const parsed = JSON.parse(body) as { messages?: unknown };
      if (!Array.isArray(parsed.messages)) {
        return {};
      }

      const responseMessageTypes: string[] = [];
      const playerStateTicks: number[] = [];
      let chunkSnapshotCount = 0;
      for (const message of parsed.messages) {
        if (typeof message !== "object" || message === null) {
          continue;
        }

        const record = message as Record<string, unknown>;
        if (typeof record.type === "string") {
          responseMessageTypes.push(record.type);
          if (record.type === "chunk_snapshot") {
            chunkSnapshotCount++;
          }
        }

        const state = record.state;
        if (record.type === "player_state" && typeof state === "object" && state !== null) {
          const tick = (state as Record<string, unknown>).tick;
          if (typeof tick === "number") {
            playerStateTicks.push(tick);
          }
        }
      }

      return {
        responseMessageTypes,
        responseMessageCount: parsed.messages.length,
        chunkSnapshotCount,
        playerStateTicks,
      };
    };

    d5Window.__mcloneD5FetchRecords = records;
    d5Window.__mcloneD5FetchRecorderFlush = async () => {
      await Promise.allSettled(pendingBodies);
    };
    window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const url = typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;
      const command = classify(url);
      if (command === undefined) {
        return await originalFetch(input, init);
      }

      const body = typeof init?.body === "string" ? init.body : "";
      const record: BrowserRecord = {
        command,
        method: init?.method ?? "GET",
        url: new URL(url, window.location.href).href,
        startMs: performance.timeOrigin + performance.now(),
        requestBytes: byteLength(body),
      };
      records.push(record);
      try {
        const response = await originalFetch(input, init);
        record.status = response.status;
        record.endMs = performance.timeOrigin + performance.now();
        const pendingBody = response.clone().text()
          .then((text) => {
            record.responseBytes = byteLength(text);
            Object.assign(record, extractMetrics(text));
          })
          .catch((error: unknown) => {
            record.error = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
          });
        pendingBodies.push(pendingBody);
        return response;
      } catch (error) {
        record.endMs = performance.timeOrigin + performance.now();
        record.error = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
        throw error;
      }
    };
  });
}

async function readBrowserFetchRecords(page: Page): Promise<MutableNetworkRecord[]> {
  return await page.evaluate(async () => {
    const d5Window = window as DebugWindow;
    await d5Window.__mcloneD5FetchRecorderFlush?.();
    return d5Window.__mcloneD5FetchRecords ?? [];
  });
}

function toTransportRecord(record: MutableNetworkRecord): D5TransportRequestRecord {
  return {
    command: record.command,
    method: record.method,
    url: record.url,
    status: record.status,
    startMs: record.startMs,
    endMs: record.endMs,
    durationMs: record.endMs === undefined ? undefined : record.endMs - record.startMs,
    requestBytes: record.requestBytes,
    responseBytes: record.responseBytes,
    responseMessageTypes: record.responseMessageTypes,
    responseMessageCount: record.responseMessageCount,
    chunkSnapshotCount: record.chunkSnapshotCount,
    playerStateTicks: record.playerStateTicks,
    error: record.error,
  };
}

function summarizeTransport(records: readonly MutableNetworkRecord[]): D5TransportSummary {
  const transportRecords = records.map(toTransportRecord);
  const byCommand: D5TransportSummary["byCommand"] = {};
  for (const command of ["open_world", "set_chunk_view", "set_player_input", "poll_world_updates"] as const) {
    const commandRecords = transportRecords.filter((record) => record.command === command);
    if (commandRecords.length === 0) {
      continue;
    }

    byCommand[command] = {
      requestCount: commandRecords.length,
      failedRequestCount: commandRecords.filter((record) => record.error !== undefined).length,
      requestDurationMs: summarizeNumbers(commandRecords.flatMap((record) => (
        record.durationMs === undefined ? [] : [record.durationMs]
      ))),
      requestBytes: commandRecords.reduce((total, record) => total + record.requestBytes, 0),
      responseBytes: commandRecords.reduce((total, record) => total + (record.responseBytes ?? 0), 0),
      responseMessageCount: commandRecords.reduce((total, record) => total + (record.responseMessageCount ?? 0), 0),
      chunkSnapshotCount: commandRecords.reduce((total, record) => total + (record.chunkSnapshotCount ?? 0), 0),
      playerStateMessageCount: commandRecords.reduce((total, record) => total + (record.playerStateTicks?.length ?? 0), 0),
    };
  }

  return {
    requestCount: transportRecords.length,
    failedRequestCount: transportRecords.filter((record) => record.error !== undefined).length,
    responseBytes: transportRecords.reduce((total, record) => total + (record.responseBytes ?? 0), 0),
    byCommand,
    requests: transportRecords,
  };
}

async function readEnvironment(
  page: Page,
  commit: string,
  browserName: string,
  browserVersion: string,
): Promise<D5Environment> {
  const browserEnvironment = await page.evaluate(async () => {
    const adapter = await navigator.gpu?.requestAdapter();
    const info = adapter?.info;
    return {
      userAgent: navigator.userAgent,
      webGpuAdapter: info === undefined
        ? "unknown"
        : [info.vendor, info.architecture, info.device, info.description].filter(Boolean).join(" / ") || "unknown",
      devicePixelRatio: window.devicePixelRatio,
      crossOriginIsolated: window.crossOriginIsolated,
      sharedArrayBufferAvailable: typeof SharedArrayBuffer !== "undefined",
    };
  });

  return {
    commit,
    os: `${platform()} ${release()}`,
    arch: arch(),
    nodeVersion: process.version,
    browserName,
    browserVersion,
    ...browserEnvironment,
  };
}

function summarizeMainThread(
  samples: readonly D5DebugStateSnapshot[],
  startState: DebugRuntimeState,
  endState: DebugRuntimeState,
) {
  const queueSamples = samples.flatMap((sample) => sample.renderQueueStats === undefined ? [] : [sample.renderQueueStats]);
  return {
    startQueue: startState.renderQueueStats,
    endQueue: endState.renderQueueStats,
    maxPendingVisibleChunkCompileCount: Math.max(0, ...queueSamples.map((stats) => stats.pendingVisibleChunkCompileCount)),
    maxQueuedChunkBuildCount: Math.max(0, ...queueSamples.map((stats) => stats.queuedChunkBuildCount)),
    maxActiveChunkBuildCount: Math.max(0, ...queueSamples.map((stats) => stats.activeChunkBuildCount)),
    maxRenderedChunkCount: Math.max(0, ...queueSamples.map((stats) => stats.renderedChunkCount)),
  };
}

function traceTimestampMsToUs(ms: number): number {
  return Math.round(ms * 1000);
}

function createTraceEvents(
  report: D5TraversalReport,
  probe: D5FrameProbeResult,
  traversalStartMs: number,
  traversalEndMs: number,
): readonly TraceEvent[] {
  const events: TraceEvent[] = [{
    name: "d5.traversal",
    ph: "X",
    ts: traceTimestampMsToUs(traversalStartMs),
    dur: traceTimestampMsToUs(traversalEndMs - traversalStartMs),
    pid: 1,
    tid: 1,
    args: {
      chunkViewChanges: report.traversal.chunkViewChanges,
      frameGapP95Ms: report.framePacing.p95,
      frameGapMaxMs: report.framePacing.max,
      gatePassed: report.gates.passed,
      gateFailures: report.gates.failures,
      lightingMaxCommandMs: report.lightingWorker.maxWorkerCommandDurationMs,
      lightingMaxPropagationSliceMs: report.lightingWorker.maxPropagationSliceMs,
    },
  }];

  for (const marker of probe.markers) {
    events.push({
      name: `d5.marker.${marker.name}`,
      ph: "i",
      s: "t",
      ts: traceTimestampMsToUs(probe.timeOriginMs + marker.atMs),
      pid: 1,
      tid: 1,
    });
  }

  for (const task of probe.longTasks) {
    events.push({
      name: "d5.long_task",
      ph: "X",
      ts: traceTimestampMsToUs(probe.timeOriginMs + task.startTimeMs),
      dur: traceTimestampMsToUs(task.durationMs),
      pid: 1,
      tid: 1,
      args: {
        nearestMarker: task.nearestMarker,
      },
    });
  }

  for (const request of report.transport.requests) {
    if (request.durationMs === undefined) {
      continue;
    }

    events.push({
      name: `d5.http.${request.command}`,
      ph: "X",
      ts: traceTimestampMsToUs(request.startMs),
      dur: traceTimestampMsToUs(request.durationMs),
      pid: 2,
      tid: 1,
      args: {
        status: request.status,
        requestBytes: request.requestBytes,
        responseBytes: request.responseBytes,
      },
    });
  }

  for (const [index, chunk] of report.traversal.chunkPath.entries()) {
    const sample = report.samples.find((candidate) => (
      candidate.playerChunk !== undefined
      && candidate.playerChunk[0] === chunk[0]
      && candidate.playerChunk[1] === chunk[1]
    ));
    events.push({
      name: "d5.chunk_view",
      ph: "i",
      s: "t",
      ts: traceTimestampMsToUs(traversalStartMs + (sample?.atMs ?? 0)),
      pid: 1,
      tid: 2,
      args: {
        index,
        chunk,
      },
    });
  }

  return events;
}

test("D5 remote traversal measurement harness", async ({ page, remoteWorldHostUrl, browserName, browser }) => {
  const pageErrors: string[] = [];
  const consoleErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(formatUnknownError(error)));
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });

  await page.setViewportSize(D5_CONFIG.viewport);
  await installBrowserFetchRecorder(page);
  await page.goto(createDebugUrl(remoteWorldHostUrl), { waitUntil: "load" });
  await waitForDebugReady(page);
  const startState = await waitForLoadedSettledFrame(page, EXPECTED_LOADED_CHUNK_COUNT);
  expect(startState.error).toBeUndefined();
  expect(startState.worldTransport).toBe("remote");
  expect(startState.viewDistance).toBe(D5_CONFIG.viewDistance);
  expect(startState.renderDistance).toBe(D5_CONFIG.renderDistance);
  expect(startState.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(startState.renderWorldCounters?.ingestBatchCount ?? 0).toBeGreaterThan(0);
  expect(startState.renderWorldCounters?.meshBuildRequestCount ?? 0).toBeGreaterThan(0);
  expect(startState.renderWorldCounters?.meshCompletionCount ?? 0).toBeGreaterThan(0);
  expect(startState.renderWorldCounters?.mainThreadGpuUploadCount ?? 0).toBeGreaterThan(0);

  await page.locator("#renderer").screenshot({ path: START_SCREENSHOT_PATH });

  const commit = readGitCommit();
  const environment = await readEnvironment(page, commit, browserName, browser.version());
  const artifacts: D5Artifacts = {
    startScreenshot: START_SCREENSHOT_PATH,
    endScreenshot: END_SCREENSHOT_PATH,
    report: REPORT_PATH,
    trace: TRACE_PATH,
  };
  const samples: D5DebugStateSnapshot[] = [];
  const chunkPath: Array<readonly [number, number]> = [];
  const initialChunk = chunkPair(startState);
  if (initialChunk === undefined) {
    throw new Error("D5 traversal start state did not include a player chunk");
  }
  chunkPath.push(initialChunk);
  samples.push(createSnapshot(0, startState));

  await startFrameProbe(page);
  await markFrameProbe(page, "traversal_start");
  const traversalStartMs = performance.timeOrigin + performance.now();
  await setTraversalInput(page, true);

  let lastChunk = initialChunk;
  while (true) {
    await page.waitForTimeout(D5_CONFIG.sampleIntervalMs);
    const nowMs = performance.timeOrigin + performance.now();
    const elapsedMs = nowMs - traversalStartMs;
    const state = await readDebugState(page);
    expect(state.error).toBeUndefined();
    samples.push(createSnapshot(elapsedMs, state));

    const nextChunk = chunkPair(state);
    if (nextChunk !== undefined && !sameChunkPair(lastChunk, nextChunk)) {
      chunkPath.push(nextChunk);
      lastChunk = nextChunk;
      await markFrameProbe(page, `chunk_view_${chunkPath.length - 1}`);
    }

    const chunkViewChanges = Math.max(0, chunkPath.length - 1);
    if (elapsedMs >= D5_CONFIG.targetDurationMs && chunkViewChanges >= D5_CONFIG.minChunkBoundaryCrossings) {
      break;
    }

    if (elapsedMs >= D5_CONFIG.maxDurationMs) {
      throw new Error(
        `D5 traversal crossed ${chunkViewChanges.toString()} chunk boundaries in ${elapsedMs.toFixed(0)}ms; expected at least ${D5_CONFIG.minChunkBoundaryCrossings.toString()}`,
      );
    }
  }

  await setTraversalInput(page, false);
  const traversalEndMs = performance.timeOrigin + performance.now();
  await markFrameProbe(page, "traversal_end");
  const probe = await stopFrameProbe(page);

  const endState = await waitForLoadedSettledFrame(page, EXPECTED_LOADED_CHUNK_COUNT);
  samples.push(createSnapshot(traversalEndMs - traversalStartMs, endState));
  await page.locator("#renderer").screenshot({ path: END_SCREENSHOT_PATH });
  const networkRecords = await readBrowserFetchRecords(page);

  const endChunk = chunkPair(endState);
  if (endChunk === undefined) {
    throw new Error("D5 traversal end state did not include a player chunk");
  }

  const renderWorldDelta = diffRenderWorldCounters(startState.renderWorldCounters, endState.renderWorldCounters);
  const transportRecords = networkRecords.map(toTransportRecord);
  const traversalTransportRecords = transportRecords.filter((record) => (
    record.startMs >= traversalStartMs && record.startMs <= traversalEndMs
  ));
  expect(chunkPath.length - 1).toBeGreaterThanOrEqual(D5_CONFIG.minChunkBoundaryCrossings);
  expect(endState.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(isRenderQueueSettled(endState)).toBe(true);
  expect(renderWorldDelta?.ingestBatchCount ?? 0).toBeGreaterThan(0);
  expect(renderWorldDelta?.meshBuildRequestCount ?? 0).toBeGreaterThan(0);
  expect(renderWorldDelta?.meshCompletionCount ?? 0).toBeGreaterThan(0);
  expect(renderWorldDelta?.mainThreadGpuUploadCount ?? 0).toBeGreaterThan(0);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter(isWebGpuConsoleError)).toEqual([]);

  const traversalDurationMs = traversalEndMs - traversalStartMs;
  const reportWithoutGates: D5TraversalReportWithoutGates = {
    schemaVersion: D5_TRAVERSAL_REPORT_SCHEMA_VERSION,
    commit,
    config: D5_CONFIG,
    environment,
    artifacts,
    traversal: {
      durationMs: traversalDurationMs,
      chunkViewChanges: chunkPath.length - 1,
      chunkPath,
      startChunk: initialChunk,
      endChunk,
      startPosition: startState.playerPosition,
      endPosition: endState.playerPosition,
      finalLoadedChunkCount: endState.loadedChunkCount,
      expectedLoadedChunkCount: endState.expectedLoadedChunkCount,
    },
    framePacing: summarizeFramePacing(probe, traversalDurationMs),
    hostResponsiveness: summarizeHostResponsiveness(
      traversalTransportRecords,
      samples,
    ),
    transport: summarizeTransport(networkRecords),
    renderWorld: {
      start: startState.renderWorldCounters,
      end: endState.renderWorldCounters,
      delta: renderWorldDelta,
    },
    mainThread: summarizeMainThread(samples, startState, endState),
    lightingWorker: summarizeLightingWorker(samples),
    samples,
    decision: {
      status: "regression_gate",
      nextAction: "Use this gate for future loading/lighting changes, and add deeper phase timers only when a gate fails without an obvious owner.",
      notes: [
        "This run measures the live remote traversal path and fails if responsiveness, lighting-worker slices, render-world ingest, or GPU upload behavior crosses the configured gate.",
        "The scripted input holds Space with KeyW so the current debug flight controls do not drive the camera below the world while pitched down.",
        "It intentionally does not start push transport, SharedArrayBuffer, or render-world subworkers.",
      ],
    },
  };
  const gates = evaluateD5Gates(reportWithoutGates, D5_GATE_THRESHOLDS);
  const report: D5TraversalReport = {
    ...reportWithoutGates,
    gates,
  };

  await writeFile(REPORT_PATH, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  await writeFile(TRACE_PATH, `${JSON.stringify({
    traceEvents: createTraceEvents(report, probe, traversalStartMs, traversalEndMs),
  }, null, 2)}\n`, "utf8");
  expect(gates.failures).toEqual([]);
});
