import { Vec3 } from "../world/phys/vec3";
import type { NormalizedWorldEngineConfig, SetPlayerInputRequest } from "../runtime/protocol/world-messages";
import { getDefaultRenderDistance, getExpectedLoadedChunkCount } from "./browser-render-config";
import type { CameraState } from "./game-renderer";
import type { RenderSceneQueueStats, RenderWorldPerformanceCounters } from "./scene-setup";

export interface GeneratedWorldSmokeScenario {
  readonly seed: bigint;
  readonly preset: "browser_smoke";
  readonly width: number;
  readonly height: number;
  readonly outputPath: string;
  readonly renderTargetFormat: GPUTextureFormat;
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly engineConfig: NormalizedWorldEngineConfig;
  readonly camera: CameraState;
  readonly skyColor: Vec3;
  readonly clearColorScale: number;
  readonly fogColorHex: string;
  readonly minimumNonClearPixels: number;
  readonly playerInput: SetPlayerInputRequest["input"];
}

export interface GeneratedWorldSmokeResultLike {
  readonly worldTransport?: "worker" | "remote";
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount: number;
  readonly viewDistance: number;
  readonly renderDistance: number;
  readonly lightingMode: string;
  readonly liquidSimulationMode: string;
  readonly solidDrawCount: number;
  readonly nonClearPixels?: number;
  readonly centerPixel?: readonly number[];
  readonly terrainPixel?: readonly number[];
  readonly renderWorldCounters: RenderWorldPerformanceCounters;
  readonly renderQueueStats: RenderSceneQueueStats;
  readonly sessionId?: string;
  readonly playerId?: string;
  readonly playerName?: string;
  readonly playerProfileId?: string;
  readonly sessionRevision?: number;
  readonly playerInputSequence?: number;
  readonly playerStateRevision?: number;
  readonly playerTick?: number;
  readonly playerPosition?: readonly number[];
}

export interface GeneratedWorldSmokeValidationOptions {
  readonly expectedWorldTransport?: "worker" | "remote";
  readonly requireReadback?: boolean;
  readonly requirePlayerInput?: boolean;
}

export const GENERATED_WORLD_SMOKE_SCENARIO: GeneratedWorldSmokeScenario = {
  seed: 12_345n,
  preset: "browser_smoke",
  width: 256,
  height: 256,
  outputPath: "/tmp/mclone-deno-generated-world-smoke.png",
  renderTargetFormat: "rgba8unorm",
  viewDistance: 1,
  renderDistance: getDefaultRenderDistance(1),
  engineConfig: {
    lightingMode: "none",
    liquidSimulationMode: "none",
  },
  camera: {
    position: new Vec3(960.5, 132.0, -8127.5),
    xRot: 60.0,
    yRot: 225.0,
  },
  skyColor: new Vec3(0x8f / 255, 0xb8 / 255, 0xff / 255),
  clearColorScale: 1,
  fogColorHex: "8fb8ff",
  minimumNonClearPixels: 1_000,
  playerInput: {
    sequence: 1,
    moveX: 1,
    moveY: 0,
    moveZ: 0,
    yaw: 180,
    pitch: 60,
  },
};

export function getGeneratedWorldSmokeExpectedLoadedChunkCount(
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): number {
  return getExpectedLoadedChunkCount(scenario.viewDistance);
}

export function createGeneratedWorldSmokeSearchParams(
  extra: Record<string, string>,
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): URLSearchParams {
  return new URLSearchParams({
    viewDistance: scenario.viewDistance.toString(),
    renderDistance: scenario.renderDistance.toString(),
    lightingMode: scenario.engineConfig.lightingMode,
    liquidSimulationMode: scenario.engineConfig.liquidSimulationMode,
    fogColor: scenario.fogColorHex,
    cameraX: scenario.camera.position.x.toString(),
    cameraY: scenario.camera.position.y.toString(),
    cameraZ: scenario.camera.position.z.toString(),
    cameraYaw: scenario.camera.yRot.toString(),
    cameraPitch: scenario.camera.xRot.toString(),
    ...extra,
  });
}

export function validateGeneratedWorldSmokeResult(
  result: GeneratedWorldSmokeResultLike,
  options: GeneratedWorldSmokeValidationOptions = {},
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): string[] {
  const errors: string[] = [];
  const expectedLoadedChunkCount = getGeneratedWorldSmokeExpectedLoadedChunkCount(scenario);
  if (options.expectedWorldTransport !== undefined && result.worldTransport !== options.expectedWorldTransport) {
    errors.push(`expected worldTransport=${options.expectedWorldTransport}, got ${result.worldTransport ?? "undefined"}`);
  }
  if (result.loadedChunkCount !== expectedLoadedChunkCount) {
    errors.push(`expected loadedChunkCount=${expectedLoadedChunkCount.toString()}, got ${result.loadedChunkCount.toString()}`);
  }
  if (result.expectedLoadedChunkCount !== expectedLoadedChunkCount) {
    errors.push(`expected expectedLoadedChunkCount=${expectedLoadedChunkCount.toString()}, got ${result.expectedLoadedChunkCount.toString()}`);
  }
  if (result.viewDistance !== scenario.viewDistance) {
    errors.push(`expected viewDistance=${scenario.viewDistance.toString()}, got ${result.viewDistance.toString()}`);
  }
  if (result.renderDistance !== scenario.renderDistance) {
    errors.push(`expected renderDistance=${scenario.renderDistance.toString()}, got ${result.renderDistance.toString()}`);
  }
  if (result.lightingMode !== scenario.engineConfig.lightingMode) {
    errors.push(`expected lightingMode=${scenario.engineConfig.lightingMode}, got ${result.lightingMode}`);
  }
  if (result.liquidSimulationMode !== scenario.engineConfig.liquidSimulationMode) {
    errors.push(`expected liquidSimulationMode=${scenario.engineConfig.liquidSimulationMode}, got ${result.liquidSimulationMode}`);
  }
  if (result.solidDrawCount <= 0) {
    errors.push("expected at least one solid draw");
  }
  if (result.renderWorldCounters.ingestBatchCount <= 0) {
    errors.push("expected at least one render-world ingest batch");
  }
  if (result.renderWorldCounters.meshBuildRequestCount <= 0) {
    errors.push("expected at least one render-world mesh build request");
  }
  if (result.renderWorldCounters.meshCompletionCount <= 0) {
    errors.push("expected at least one render-world mesh completion");
  }
  if (result.renderWorldCounters.mainThreadGpuUploadCount <= 0) {
    errors.push("expected at least one main-thread GPU upload");
  }
  if (result.renderWorldCounters.meshNotReadyResponseCount < 0) {
    errors.push("expected non-negative mesh-not-ready count");
  }
  if (result.renderQueueStats.renderedChunkCount <= 0) {
    errors.push("expected rendered chunks");
  }
  if (result.renderQueueStats.pendingVisibleChunkCompileCount !== 0) {
    errors.push(`expected no pending visible chunk compiles, got ${result.renderQueueStats.pendingVisibleChunkCompileCount.toString()}`);
  }
  if (result.renderQueueStats.queuedChunkBuildCount !== 0) {
    errors.push(`expected no queued chunk builds, got ${result.renderQueueStats.queuedChunkBuildCount.toString()}`);
  }
  if (result.renderQueueStats.activeChunkBuildCount !== 0) {
    errors.push(`expected no active chunk builds, got ${result.renderQueueStats.activeChunkBuildCount.toString()}`);
  }

  if (options.requireReadback) {
    if (result.nonClearPixels === undefined) {
      errors.push("expected nonClearPixels in readback result");
    } else if (result.nonClearPixels < scenario.minimumNonClearPixels) {
      errors.push(`expected at least ${scenario.minimumNonClearPixels.toString()} non-clear pixels, got ${result.nonClearPixels.toString()}`);
    }
    validatePixel("centerPixel", result.centerPixel, errors);
    validatePixel("terrainPixel", result.terrainPixel, errors);
  }

  if (options.requirePlayerInput) {
    if (result.sessionId === undefined) {
      errors.push("expected sessionId");
    }
    if (result.playerId === undefined) {
      errors.push("expected playerId");
    }
    if (result.sessionId !== undefined && result.playerId !== undefined && result.sessionId === result.playerId) {
      errors.push("expected playerId to differ from sessionId");
    }
    if (result.playerName !== "Player") {
      errors.push(`expected playerName=Player, got ${result.playerName ?? "undefined"}`);
    }
    if ((result.sessionRevision ?? 0) <= 0) {
      errors.push("expected positive sessionRevision");
    }
    if (result.playerInputSequence !== scenario.playerInput.sequence) {
      errors.push(`expected playerInputSequence=${scenario.playerInput.sequence.toString()}, got ${result.playerInputSequence?.toString() ?? "undefined"}`);
    }
    if ((result.playerStateRevision ?? 0) <= 0) {
      errors.push("expected positive playerStateRevision");
    }
    if ((result.playerTick ?? 0) <= 0) {
      errors.push("expected positive playerTick");
    }
    if (result.playerPosition?.length !== 3) {
      errors.push("expected 3D playerPosition");
    }
  }

  return errors;
}

function validatePixel(label: string, pixel: readonly number[] | undefined, errors: string[]): void {
  if (pixel === undefined) {
    errors.push(`expected ${label}`);
    return;
  }
  if (pixel.length !== 4) {
    errors.push(`expected ${label} to have four RGBA channels`);
  }
}
