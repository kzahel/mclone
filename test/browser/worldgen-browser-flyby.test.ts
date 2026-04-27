import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { arch, platform, release } from "node:os";
import process from "node:process";
import { performance } from "node:perf_hooks";
import type { Page } from "@playwright/test";
import type { OpenWorldPreset, WorldPerformanceSnapshot, WorldgenPerformanceCounters } from "../../src/runtime/protocol/world-messages";
import { expect, test } from "./remote-world-host-fixture";
import {
  diffRenderWorldCounters,
  summarizeFramePacing,
  summarizeHostResponsiveness,
  summarizeLightingWorker,
  summarizeNumbers,
  type D5DebugStateSnapshot,
  type D5FrameProbeResult,
  type D5LongTaskRecord,
  type D5PhaseMarker,
  type D5RenderQueueStats,
  type D5RenderWorldCounters,
  type D5TransportCommand,
  type D5TransportRequestRecord,
} from "./d5-traversal-report";

const PRESETS = ["flat_grass", "default"] as const satisfies readonly OpenWorldPreset[];
const VIEW_DISTANCE = 1;
const EXPECTED_LOADED_CHUNK_COUNT = 25;
const TARGET_CHUNK_BOUNDARY_CROSSINGS = 6;
const MAX_TRAVERSAL_MS = 18_000;
const SAMPLE_INTERVAL_MS = 100;
const VIEWPORT = { width: 1280, height: 720 };

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
  readonly cameraPosition?: readonly [number, number, number];
  readonly cameraYaw?: number;
  readonly cameraPitch?: number;
  readonly frameCount: number;
  readonly renderWorldCounters?: D5RenderWorldCounters;
  readonly renderQueueStats?: D5RenderQueueStats;
  readonly worldPerformance?: WorldPerformanceSnapshot;
  readonly error?: string;
}

interface DebugInjectedInput {
  readonly locked?: boolean;
  readonly heldKeys?: readonly string[];
  readonly moveForward?: boolean;
}

interface DebugRuntimeController {
  readonly state: DebugRuntimeState;
  setInjectedInput(input: DebugInjectedInput | null): void;
}

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

interface MutableSocketRecord {
  command: D5TransportCommand | "websocket_push";
  kind: "request" | "push";
  requestId?: number;
  sequence?: number;
  url: string;
  startMs: number;
  endMs?: number;
  requestBytes: number;
  responseBytes?: number;
  responseMessageTypes?: string[];
  responseMessageCount?: number;
  chunkSnapshotCount?: number;
  playerStateTicks?: number[];
  error?: string;
}

interface FrameProbe {
  mark(name: string): void;
  stop(): D5FrameProbeResult;
}

type DebugWindow = Window & {
  __mcloneDebug?: DebugRuntimeController;
  __mcloneWorldgenFrameProbe?: FrameProbe;
  __mcloneWorldgenFetchRecords?: MutableNetworkRecord[];
  __mcloneWorldgenSocketRecords?: MutableSocketRecord[];
  __mcloneWorldgenFetchRecorderFlush?: () => Promise<void>;
};

interface WorldgenBrowserFlybyReport {
  readonly schemaVersion: 1;
  readonly commit: string;
  readonly preset: OpenWorldPreset;
  readonly environment: Awaited<ReturnType<typeof readEnvironment>>;
  readonly artifacts: {
    readonly startScreenshot: string;
    readonly endScreenshot: string;
    readonly report: string;
  };
  readonly config: {
    readonly seed: string;
    readonly viewDistance: number;
    readonly lightingMode: "none";
    readonly liquidSimulationMode: "none";
    readonly movementMode: "freecam";
    readonly targetChunkBoundaryCrossings: number;
    readonly sampleIntervalMs: number;
  };
  readonly traversal: {
    readonly durationMs: number;
    readonly chunkViewChanges: number;
    readonly chunkPath: readonly (readonly [number, number])[];
    readonly startCameraPosition?: readonly [number, number, number];
    readonly endCameraPosition?: readonly [number, number, number];
    readonly finalLoadedChunkCount: number;
    readonly expectedLoadedChunkCount?: number;
  };
  readonly framePacing: ReturnType<typeof summarizeFramePacing>;
  readonly hostResponsiveness: ReturnType<typeof summarizeHostResponsiveness>;
  readonly renderWorld: {
    readonly start?: D5RenderWorldCounters;
    readonly end?: D5RenderWorldCounters;
    readonly delta?: D5RenderWorldCounters;
  };
  readonly mainThread: ReturnType<typeof summarizeMainThread>;
  readonly lightingWorker: ReturnType<typeof summarizeLightingWorker>;
  readonly worldgen: {
    readonly start?: WorldgenPerformanceCounters;
    readonly end?: WorldgenPerformanceCounters;
    readonly delta?: WorldgenPerformanceCounters;
    readonly topPhases: readonly {
      readonly name: string;
      readonly count: number;
      readonly totalMs: number;
      readonly maxMs: number;
    }[];
  };
  readonly transport: {
    readonly requestCount: number;
    readonly failedRequestCount: number;
    readonly responseBytes: number;
    readonly byCommand: Partial<Record<D5TransportCommand, {
      readonly requestCount: number;
      readonly requestDurationMs: ReturnType<typeof summarizeNumbers>;
      readonly chunkSnapshotCount: number;
      readonly responseBytes: number;
    }>>;
    readonly requests: readonly D5TransportRequestRecord[];
  };
  readonly socketTransport: {
    readonly requestCount: number;
    readonly pushCount: number;
    readonly byCommand: Partial<Record<D5TransportCommand, {
      readonly requestCount: number;
      readonly requestDurationMs: ReturnType<typeof summarizeNumbers>;
      readonly chunkSnapshotCount: number;
      readonly responseBytes: number;
    }>>;
    readonly pushChunkSnapshotCount: number;
    readonly pushMessageCount: number;
    readonly records: readonly MutableSocketRecord[];
  };
  readonly samples: readonly D5DebugStateSnapshot[];
  readonly consoleErrors: readonly string[];
  readonly pageErrors: readonly string[];
}

function createDebugUrl(remoteWorldHostUrl: string, preset: OpenWorldPreset): string {
  return `/?mode=debug&${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
    seed: "12345",
    preset,
    movementMode: "freecam",
    cameraX: "8.5",
    cameraY: "76",
    cameraZ: "8.5",
    cameraYaw: "270",
    cameraPitch: "0",
    preserveInitialCamera: "1",
    viewDistance: VIEW_DISTANCE.toString(),
    renderDistance: "112",
    lightingMode: "none",
    liquidSimulationMode: "none",
    worldStorageMode: "none",
    fogColor: "8fb8ff",
  }).toString()}`;
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
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

function cameraChunkPair(state: DebugRuntimeState): readonly [number, number] | undefined {
  if (state.chunkViewCenterX === undefined || state.chunkViewCenterZ === undefined) {
    return undefined;
  }
  return [state.chunkViewCenterX, state.chunkViewCenterZ];
}

function sameChunkPair(left: readonly [number, number] | undefined, right: readonly [number, number]): boolean {
  return left !== undefined && left[0] === right[0] && left[1] === right[1];
}

function createSnapshot(atMs: number, state: DebugRuntimeState): D5DebugStateSnapshot {
  return {
    atMs,
    playerTick: state.playerTick,
    playerPosition: state.playerPosition,
    playerChunk: state.playerChunkX === undefined || state.playerChunkZ === undefined
      ? undefined
      : [state.playerChunkX, state.playerChunkZ],
    chunkViewCenter: cameraChunkPair(state),
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
  await page.waitForFunction(() => (window as DebugWindow).__mcloneDebug !== undefined, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => {
    const state = (window as DebugWindow).__mcloneDebug?.state;
    return state?.ready === true || state?.error !== undefined;
  }, undefined, { timeout: 60_000 });
}

async function waitForLoadedSettledFrame(page: Page): Promise<DebugRuntimeState> {
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
  }, EXPECTED_LOADED_CHUNK_COUNT, { timeout: 90_000 });

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  expect(state.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(isRenderQueueSettled(state)).toBe(true);
  return state;
}

async function waitForSettledFrameOrCurrent(page: Page, timeoutMs: number): Promise<DebugRuntimeState> {
  try {
    await page.waitForFunction(() => {
      const state = (window as DebugWindow).__mcloneDebug?.state;
      if (state === undefined || state.error !== undefined) {
        return true;
      }
      const stats = state.renderQueueStats;
      return state.loadedChunkCount > 0
        && stats !== undefined
        && stats.renderedChunkCount > 0
        && stats.pendingVisibleChunkCompileCount === 0
        && stats.queuedChunkBuildCount === 0
        && stats.activeChunkBuildCount === 0;
    }, undefined, { timeout: timeoutMs });
  } catch (error) {
    if (!(error instanceof Error) || !error.message.includes("Timeout")) {
      throw error;
    }
  }

  const state = await readDebugState(page);
  expect(state.error).toBeUndefined();
  return state;
}

async function setTraversalInput(page: Page, enabled: boolean): Promise<void> {
  await page.evaluate((isEnabled) => {
    const debug = (window as DebugWindow).__mcloneDebug;
    if (debug === undefined) {
      throw new Error("window.__mcloneDebug is not installed");
    }
    debug.setInjectedInput(isEnabled ? { locked: true, moveForward: true } : null);
  }, enabled);
}

async function startFrameProbe(page: Page): Promise<void> {
  await page.evaluate(() => {
    const probeWindow = window as DebugWindow;
    const frameTimesMs: number[] = [];
    const longTasks: D5LongTaskRecord[] = [];
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

    probeWindow.__mcloneWorldgenFrameProbe = {
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
    const probe = (window as DebugWindow).__mcloneWorldgenFrameProbe;
    if (probe === undefined) {
      throw new Error("window.__mcloneWorldgenFrameProbe is not installed");
    }
    probe.mark(markerName);
  }, name);
}

async function stopFrameProbe(page: Page): Promise<D5FrameProbeResult> {
  return await page.evaluate(() => {
    const probeWindow = window as DebugWindow;
    const probe = probeWindow.__mcloneWorldgenFrameProbe;
    if (probe === undefined) {
      throw new Error("window.__mcloneWorldgenFrameProbe is not installed");
    }
    const result = probe.stop();
    probeWindow.__mcloneWorldgenFrameProbe = undefined;
    return result;
  });
}

async function installBrowserFetchRecorder(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const recorderWindow = window as DebugWindow;
    const records: MutableNetworkRecord[] = [];
    const socketRecords: MutableSocketRecord[] = [];
    const pendingSocketRequests = new Map<number, MutableSocketRecord>();
    const pendingBodies: Promise<void>[] = [];
    const originalFetch = window.fetch.bind(window);
    const OriginalWebSocket = window.WebSocket;

    const classify = (urlValue: string): D5TransportCommand | undefined => {
      const url = new URL(urlValue, window.location.href);
      if (url.pathname === "/api/world/session") return "open_world";
      if (/^\/api\/world\/session\/[^/]+\/chunk-view$/.test(url.pathname)) return "set_chunk_view";
      if (/^\/api\/world\/session\/[^/]+\/player-input$/.test(url.pathname)) return "set_player_input";
      if (/^\/api\/world\/session\/[^/]+\/updates\/poll$/.test(url.pathname)) return "poll_world_updates";
      return undefined;
    };

    const byteLength = (value: string): number => new TextEncoder().encode(value).byteLength;
    const extractMetrics = (body: string): Partial<MutableNetworkRecord> => {
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

    recorderWindow.__mcloneWorldgenFetchRecords = records;
    recorderWindow.__mcloneWorldgenSocketRecords = socketRecords;
    recorderWindow.__mcloneWorldgenFetchRecorderFlush = async () => {
      await Promise.allSettled(pendingBodies);
    };

    window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      const command = classify(url);
      if (command === undefined) {
        return await originalFetch(input, init);
      }

      const body = typeof init?.body === "string" ? init.body : "";
      const record: MutableNetworkRecord = {
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
            record.error = formatUnknownError(error);
          });
        pendingBodies.push(pendingBody);
        return response;
      } catch (error) {
        record.endMs = performance.timeOrigin + performance.now();
        record.error = formatUnknownError(error);
        throw error;
      }
    };

    const classifySocketMessage = (value: unknown): D5TransportCommand | undefined => {
      if (typeof value !== "object" || value === null) {
        return undefined;
      }
      const type = (value as Record<string, unknown>).type;
      if (type === "open_world" || type === "set_chunk_view" || type === "set_player_input" || type === "poll_world_updates") {
        return type;
      }
      return undefined;
    };

    const extractSocketMetrics = (messages: unknown): Pick<
      MutableSocketRecord,
      "responseMessageTypes" | "responseMessageCount" | "chunkSnapshotCount" | "playerStateTicks"
    > => {
      if (!Array.isArray(messages)) {
        return {};
      }
      const responseMessageTypes: string[] = [];
      const playerStateTicks: number[] = [];
      let chunkSnapshotCount = 0;
      for (const message of messages) {
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
        responseMessageCount: messages.length,
        chunkSnapshotCount,
        playerStateTicks,
      };
    };

    class RecordingWebSocket extends OriginalWebSocket {
      public constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols);
        this.addEventListener("message", (event) => {
          if (typeof event.data !== "string") {
            return;
          }
          const endMs = performance.timeOrigin + performance.now();
          const responseBytes = byteLength(event.data);
          try {
            const frame = JSON.parse(event.data) as Record<string, unknown>;
            if (frame.kind === "response" && typeof frame.requestId === "number") {
              const record = pendingSocketRequests.get(frame.requestId);
              if (record === undefined) {
                return;
              }
              pendingSocketRequests.delete(frame.requestId);
              record.endMs = endMs;
              record.responseBytes = responseBytes;
              Object.assign(record, extractSocketMetrics(frame.messages));
            } else if (frame.kind === "push") {
              socketRecords.push({
                command: "websocket_push",
                kind: "push",
                sequence: typeof frame.sequence === "number" ? frame.sequence : undefined,
                url: this.url,
                startMs: endMs,
                endMs,
                requestBytes: 0,
                responseBytes,
                ...extractSocketMetrics(frame.messages),
              });
            }
          } catch (error) {
            socketRecords.push({
              command: "websocket_push",
              kind: "push",
              url: this.url,
              startMs: endMs,
              endMs,
              requestBytes: 0,
              responseBytes,
              error: formatUnknownError(error),
            });
          }
        });
      }

      public override send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
        if (typeof data === "string") {
          try {
            const frame = JSON.parse(data) as Record<string, unknown>;
            const command = classifySocketMessage(frame.message);
            if (frame.kind === "request" && typeof frame.requestId === "number" && command !== undefined) {
              const record: MutableSocketRecord = {
                command,
                kind: "request",
                requestId: frame.requestId,
                url: this.url,
                startMs: performance.timeOrigin + performance.now(),
                requestBytes: byteLength(data),
              };
              socketRecords.push(record);
              pendingSocketRequests.set(frame.requestId, record);
            }
          } catch {}
        }
        super.send(data);
      }
    }

    window.WebSocket = RecordingWebSocket;
  });
}

async function readBrowserFetchRecords(page: Page): Promise<MutableNetworkRecord[]> {
  return await page.evaluate(async () => {
    const recorderWindow = window as DebugWindow;
    await recorderWindow.__mcloneWorldgenFetchRecorderFlush?.();
    return recorderWindow.__mcloneWorldgenFetchRecords ?? [];
  });
}

async function readBrowserSocketRecords(page: Page): Promise<MutableSocketRecord[]> {
  return await page.evaluate(() => {
    const recorderWindow = window as DebugWindow;
    return recorderWindow.__mcloneWorldgenSocketRecords ?? [];
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

function summarizeTransport(records: readonly MutableNetworkRecord[]): WorldgenBrowserFlybyReport["transport"] {
  const transportRecords = records.map(toTransportRecord);
  const byCommand: WorldgenBrowserFlybyReport["transport"]["byCommand"] = {};
  for (const command of ["open_world", "set_chunk_view", "set_player_input", "poll_world_updates"] as const) {
    const commandRecords = transportRecords.filter((record) => record.command === command);
    if (commandRecords.length === 0) {
      continue;
    }
    byCommand[command] = {
      requestCount: commandRecords.length,
      requestDurationMs: summarizeNumbers(commandRecords.flatMap((record) => (
        record.durationMs === undefined ? [] : [record.durationMs]
      ))),
      chunkSnapshotCount: commandRecords.reduce((total, record) => total + (record.chunkSnapshotCount ?? 0), 0),
      responseBytes: commandRecords.reduce((total, record) => total + (record.responseBytes ?? 0), 0),
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

function socketRecordToTransportRecord(record: MutableSocketRecord): D5TransportRequestRecord | undefined {
  if (record.kind !== "request" || record.command === "websocket_push") {
    return undefined;
  }
  return {
    command: record.command,
    method: "WEBSOCKET",
    url: record.url,
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

function summarizeSocketTransport(records: readonly MutableSocketRecord[]): WorldgenBrowserFlybyReport["socketTransport"] {
  const requestRecords = records.filter((record) => record.kind === "request" && record.command !== "websocket_push");
  const pushRecords = records.filter((record) => record.kind === "push");
  const byCommand: WorldgenBrowserFlybyReport["socketTransport"]["byCommand"] = {};
  for (const command of ["open_world", "set_chunk_view", "set_player_input", "poll_world_updates"] as const) {
    const commandRecords = requestRecords.filter((record) => record.command === command);
    if (commandRecords.length === 0) {
      continue;
    }
    byCommand[command] = {
      requestCount: commandRecords.length,
      requestDurationMs: summarizeNumbers(commandRecords.flatMap((record) => (
        record.endMs === undefined ? [] : [record.endMs - record.startMs]
      ))),
      chunkSnapshotCount: commandRecords.reduce((total, record) => total + (record.chunkSnapshotCount ?? 0), 0),
      responseBytes: commandRecords.reduce((total, record) => total + (record.responseBytes ?? 0), 0),
    };
  }
  return {
    requestCount: requestRecords.length,
    pushCount: pushRecords.length,
    byCommand,
    pushChunkSnapshotCount: pushRecords.reduce((total, record) => total + (record.chunkSnapshotCount ?? 0), 0),
    pushMessageCount: pushRecords.reduce((total, record) => total + (record.responseMessageCount ?? 0), 0),
    records,
  };
}

async function readEnvironment(page: Page, browserName: string, browserVersion: string) {
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

function diffWorldgenCounters(
  start: WorldgenPerformanceCounters | undefined,
  end: WorldgenPerformanceCounters | undefined,
): WorldgenPerformanceCounters | undefined {
  if (start === undefined || end === undefined) {
    return undefined;
  }
  const phases: Record<string, { count: number; totalMs: number; maxMs: number }> = {};
  const phaseKeys = new Set([...Object.keys(start.phases), ...Object.keys(end.phases)]);
  for (const key of phaseKeys) {
    const before = start.phases[key];
    const after = end.phases[key];
    phases[key] = {
      count: (after?.count ?? 0) - (before?.count ?? 0),
      totalMs: (after?.totalMs ?? 0) - (before?.totalMs ?? 0),
      maxMs: after?.maxMs ?? 0,
    };
  }
  const counts: Record<string, number> = {};
  const countKeys = new Set([...Object.keys(start.counts), ...Object.keys(end.counts)]);
  for (const key of countKeys) {
    const value = (end.counts[key] ?? 0) - (start.counts[key] ?? 0);
    if (value !== 0) {
      counts[key] = value;
    }
  }
  return {
    phases,
    counts,
    maxPendingMessages: end.maxPendingMessages,
    currentPendingMessages: end.currentPendingMessages,
    activeChunkViewJobRevision: end.activeChunkViewJobRevision,
  };
}

function topWorldgenPhases(worldgen: WorldgenPerformanceCounters | undefined): WorldgenBrowserFlybyReport["worldgen"]["topPhases"] {
  if (worldgen === undefined) {
    return [];
  }
  return Object.entries(worldgen.phases)
    .map(([name, phase]) => ({ name, ...phase }))
    .filter((phase) => phase.count !== 0 || phase.totalMs !== 0)
    .sort((left, right) => right.totalMs - left.totalMs)
    .slice(0, 12);
}

async function runPreset(
  page: Page,
  browserVersion: string,
  remoteWorldHostUrl: string,
  browserName: string,
  preset: OpenWorldPreset,
): Promise<WorldgenBrowserFlybyReport> {
  const pageErrors: string[] = [];
  const consoleErrors: string[] = [];
  await page.setViewportSize(VIEWPORT);
  page.on("pageerror", (error) => pageErrors.push(formatUnknownError(error)));
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });

  const startScreenshot = `/tmp/mclone-worldgen-flyby-${preset}-start.png`;
  const endScreenshot = `/tmp/mclone-worldgen-flyby-${preset}-end.png`;
  const reportPath = `/tmp/mclone-worldgen-flyby-${preset}-report.json`;
  try {
    await page.goto(createDebugUrl(remoteWorldHostUrl, preset), { waitUntil: "load" });
    await waitForDebugReady(page);
    const startState = await waitForLoadedSettledFrame(page);
    expect(startState.worldTransport).toBe("remote");
    expect(startState.viewDistance).toBe(VIEW_DISTANCE);
    expect(startState.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
    expect(startState.renderWorldCounters?.ingestBatchCount ?? 0).toBeGreaterThan(0);
    expect(startState.renderWorldCounters?.meshBuildRequestCount ?? 0).toBeGreaterThan(0);
    expect(startState.renderWorldCounters?.meshCompletionCount ?? 0).toBeGreaterThan(0);
    expect(startState.renderWorldCounters?.mainThreadGpuUploadCount ?? 0).toBeGreaterThan(0);
    await page.locator("#renderer").screenshot({ path: startScreenshot });

    const environment = await readEnvironment(page, browserName, browserVersion);
    const commit = readGitCommit();
    const samples: D5DebugStateSnapshot[] = [createSnapshot(0, startState)];
    const chunkPath: Array<readonly [number, number]> = [];
    const initialChunk = cameraChunkPair(startState);
    if (initialChunk === undefined) {
      throw new Error("worldgen browser fly-by start state did not include a chunk view center");
    }
    chunkPath.push(initialChunk);

    await startFrameProbe(page);
    await markFrameProbe(page, "traversal_start");
    const traversalStartMs = performance.timeOrigin + performance.now();
    await setTraversalInput(page, true);

    let lastChunk = initialChunk;
    while (true) {
      await page.waitForTimeout(SAMPLE_INTERVAL_MS);
      const nowMs = performance.timeOrigin + performance.now();
      const elapsedMs = nowMs - traversalStartMs;
      const state = await readDebugState(page);
      expect(state.error).toBeUndefined();
      samples.push(createSnapshot(elapsedMs, state));
      const nextChunk = cameraChunkPair(state);
      if (nextChunk !== undefined && !sameChunkPair(lastChunk, nextChunk)) {
        chunkPath.push(nextChunk);
        lastChunk = nextChunk;
        await markFrameProbe(page, `chunk_view_${chunkPath.length - 1}`);
      }
      if (chunkPath.length - 1 >= TARGET_CHUNK_BOUNDARY_CROSSINGS) {
        break;
      }
      if (elapsedMs >= MAX_TRAVERSAL_MS) {
        throw new Error(
          `worldgen browser fly-by crossed ${(chunkPath.length - 1).toString()} chunk boundaries in ${elapsedMs.toFixed(0)}ms`,
        );
      }
    }

    await setTraversalInput(page, false);
    const traversalEndMs = performance.timeOrigin + performance.now();
    await markFrameProbe(page, "traversal_end");
    const probe = await stopFrameProbe(page);
    const endState = await waitForSettledFrameOrCurrent(page, 15_000);
    samples.push(createSnapshot(traversalEndMs - traversalStartMs, endState));
    await page.locator("#renderer").screenshot({ path: endScreenshot });
    const networkRecords = await readBrowserFetchRecords(page);
    const socketRecords = await readBrowserSocketRecords(page);
    const socketTransportRecords = socketRecords.flatMap((record) => {
      const transportRecord = socketRecordToTransportRecord(record);
      return transportRecord === undefined ? [] : [transportRecord];
    });
    const transportRecords = [
      ...networkRecords.map(toTransportRecord),
      ...socketTransportRecords,
    ];
    const traversalTransportRecords = transportRecords.filter((record) => (
      record.startMs >= traversalStartMs && record.startMs <= traversalEndMs
    ));
    const traversalDurationMs = traversalEndMs - traversalStartMs;
    const worldgenDelta = diffWorldgenCounters(
      startState.worldPerformance?.worldgen,
      endState.worldPerformance?.worldgen,
    );
    const report: WorldgenBrowserFlybyReport = {
      schemaVersion: 1,
      commit,
      preset,
      environment,
      artifacts: {
        startScreenshot,
        endScreenshot,
        report: reportPath,
      },
      config: {
        seed: "12345",
        viewDistance: VIEW_DISTANCE,
        lightingMode: "none",
        liquidSimulationMode: "none",
        movementMode: "freecam",
        targetChunkBoundaryCrossings: TARGET_CHUNK_BOUNDARY_CROSSINGS,
        sampleIntervalMs: SAMPLE_INTERVAL_MS,
      },
      traversal: {
        durationMs: traversalDurationMs,
        chunkViewChanges: chunkPath.length - 1,
        chunkPath,
        startCameraPosition: startState.cameraPosition,
        endCameraPosition: endState.cameraPosition,
        finalLoadedChunkCount: endState.loadedChunkCount,
        expectedLoadedChunkCount: endState.expectedLoadedChunkCount,
      },
      framePacing: summarizeFramePacing(probe, traversalDurationMs),
      hostResponsiveness: summarizeHostResponsiveness(traversalTransportRecords, samples),
      renderWorld: {
        start: startState.renderWorldCounters,
        end: endState.renderWorldCounters,
        delta: diffRenderWorldCounters(startState.renderWorldCounters, endState.renderWorldCounters),
      },
      mainThread: summarizeMainThread(samples, startState, endState),
      lightingWorker: summarizeLightingWorker(samples),
      worldgen: {
        start: startState.worldPerformance?.worldgen,
        end: endState.worldPerformance?.worldgen,
        delta: worldgenDelta,
        topPhases: topWorldgenPhases(worldgenDelta),
      },
      transport: summarizeTransport(networkRecords),
      socketTransport: summarizeSocketTransport(socketRecords),
      samples,
      consoleErrors,
      pageErrors,
    };
    await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");

    expect(chunkPath.length - 1).toBeGreaterThanOrEqual(TARGET_CHUNK_BOUNDARY_CROSSINGS);
    expect(endState.loadedChunkCount).toBeGreaterThan(0);
    expect(pageErrors).toEqual([]);
    expect(consoleErrors.filter((message) => /\b(WebGPU|GPUValidationError|validation)\b/i.test(message))).toEqual([]);
    return report;
  } finally {
    await page.goto("about:blank").catch(() => {});
  }
}

test("browser WebGPU worldgen fly-by compares flat_grass and default", async ({ browser, page, remoteWorldHostUrl, browserName }) => {
  const reports: WorldgenBrowserFlybyReport[] = [];
  await installBrowserFetchRecorder(page);
  for (const preset of PRESETS) {
    reports.push(await runPreset(page, browser.version(), remoteWorldHostUrl, browserName, preset));
  }

  const comparisonPath = "/tmp/mclone-worldgen-flyby-report.json";
  await writeFile(comparisonPath, `${JSON.stringify({
    schemaVersion: 1,
    reports: reports.map((report) => report.artifacts.report),
    summary: reports.map((report) => ({
      preset: report.preset,
      chunkViewChanges: report.traversal.chunkViewChanges,
      frameGapP95Ms: report.framePacing.p95,
      frameGapMaxMs: report.framePacing.max,
      longTaskCount: report.framePacing.longTaskCount,
      setChunkViewP95Ms: report.hostResponsiveness.setChunkViewAckLatencyMs.p95,
      pollUpdatesP99Ms: report.hostResponsiveness.pollWorldUpdatesLatencyMs.p99,
      snapshots: (report.transport.byCommand.poll_world_updates?.chunkSnapshotCount ?? 0)
        + report.socketTransport.pushChunkSnapshotCount
        + (report.socketTransport.byCommand.set_chunk_view?.chunkSnapshotCount ?? 0),
      socketSetChunkViewP95Ms: report.socketTransport.byCommand.set_chunk_view?.requestDurationMs.p95 ?? 0,
      socketPushCount: report.socketTransport.pushCount,
      renderWorldDelta: report.renderWorld.delta,
      worldgenCounts: report.worldgen.delta?.counts,
      topWorldgenPhases: report.worldgen.topPhases.slice(0, 5),
    })),
  }, null, 2)}\n`, "utf8");
});
