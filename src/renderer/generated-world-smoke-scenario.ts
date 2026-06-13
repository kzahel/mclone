import { SectionPos } from "../core/section-pos";
import { Vec3 } from "../world/phys/vec3";
import type { NormalizedWorldEngineConfig, SetPlayerInputRequest } from "../runtime/protocol/world-messages";
import { getDefaultRenderDistance, getExpectedLoadedChunkCount } from "./browser-render-config";
import type { CameraState } from "./game-renderer";
import {
  getGeneratedWorldRuntimeTopologyRequirement,
  validateGeneratedWorldRuntimeTopology,
  worldTransportToRuntimeWorldHost,
  type GeneratedWorldRuntimeTopology,
} from "./generated-world-runtime-topology";
import type { RenderSceneQueueStats, RenderWorldPerformanceCounters } from "./scene-setup";

export interface GeneratedWorldSmokeChunkCenter {
  readonly x: number;
  readonly z: number;
}

export interface GeneratedWorldSmokeReadbackExpectation {
  readonly outputPath?: string;
  readonly minimumNonClearPixels?: number;
}

export interface GeneratedWorldSmokeScenarioStep {
  readonly name: string;
  readonly frameIndex?: number;
  readonly camera: CameraState;
  readonly playerInput?: SetPlayerInputRequest["input"];
  readonly presentationDelayMs?: number;
  readonly expectedLoadedChunkCount?: number;
  readonly expectedChunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly readback?: GeneratedWorldSmokeReadbackExpectation;
}

export interface GeneratedWorldSmokeScenarioCadence {
  readonly intervalMs: number;
  readonly requirePlayerProgression?: boolean;
}

export interface GeneratedWorldSmokeScenario {
  readonly id: string;
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
  readonly cadence?: GeneratedWorldSmokeScenarioCadence;
  readonly steps: readonly GeneratedWorldSmokeScenarioStep[];
}

export interface GeneratedWorldSmokeResultLike {
  readonly worldTransport?: "worker" | "remote";
  readonly topology?: GeneratedWorldRuntimeTopology;
  readonly stepName?: string;
  readonly stepIndex?: number;
  readonly frameIndex?: number;
  readonly presentationDelayMs?: number;
  readonly chunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly expectedChunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkRadius?: number;
  readonly expectedPlayerInputSequence?: number;
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
  readonly steps?: readonly GeneratedWorldSmokeResultLike[];
}

export interface GeneratedWorldSmokeValidationOptions {
  readonly expectedWorldTransport?: "worker" | "remote";
  readonly requireReadback?: boolean;
  readonly requirePlayerInput?: boolean;
  readonly requireSteps?: boolean;
}

const STATIC_CAMERA: CameraState = {
  position: new Vec3(960.5, 132.0, -8127.5),
  xRot: 72.0,
  yRot: 225.0,
};

const STATIC_PLAYER_INPUT: SetPlayerInputRequest["input"] = {
  sequence: 1,
  moveX: 1,
  moveY: 0,
  moveZ: 0,
  yaw: 180,
  pitch: 60,
};

const TRANSITION_CAMERA: CameraState = {
  position: new Vec3(992.5, 136.0, -8095.5),
  xRot: 72.0,
  yRot: 225.0,
};

const TRANSITION_PLAYER_INPUT: SetPlayerInputRequest["input"] = {
  sequence: 2,
  moveX: 0,
  moveY: 0,
  moveZ: 1,
  yaw: 225,
  pitch: 60,
};

const TICK_CADENCE_INTERVAL_MS = 50;

const TICK_CADENCE_PLAYER_INPUTS: readonly SetPlayerInputRequest["input"][] = [
  {
    sequence: 1,
    moveX: 0,
    moveY: 0,
    moveZ: 1,
    yaw: 225,
    pitch: 60,
    clientTimeUs: 50_000,
    commandQuantumUs: 50_000,
    stepCount: 1,
  },
  {
    sequence: 2,
    moveX: 1,
    moveY: 0,
    moveZ: 1,
    yaw: 225,
    pitch: 60,
    clientTimeUs: 100_000,
    commandQuantumUs: 50_000,
    stepCount: 1,
  },
  {
    sequence: 3,
    moveX: -1,
    moveY: 0,
    moveZ: 0,
    yaw: 210,
    pitch: 60,
    clientTimeUs: 150_000,
    commandQuantumUs: 50_000,
    stepCount: 1,
  },
  {
    sequence: 4,
    moveX: 0,
    moveY: 0,
    moveZ: 0,
    yaw: 210,
    pitch: 60,
    clientTimeUs: 200_000,
    commandQuantumUs: 50_000,
    stepCount: 1,
  },
];

export const GENERATED_WORLD_SMOKE_SCENARIO: GeneratedWorldSmokeScenario = {
  id: "static",
  seed: 12_345n,
  preset: "browser_smoke",
  width: 1024,
  height: 768,
  outputPath: "/tmp/mclone-deno-generated-world-smoke.png",
  renderTargetFormat: "rgba8unorm",
  viewDistance: 2,
  renderDistance: getDefaultRenderDistance(2),
  engineConfig: {
    lightingMode: "none",
    liquidSimulationMode: "none",
  },
  camera: STATIC_CAMERA,
  skyColor: new Vec3(0x8f / 255, 0xb8 / 255, 0xff / 255),
  clearColorScale: 1,
  fogColorHex: "8fb8ff",
  minimumNonClearPixels: 1_000,
  playerInput: STATIC_PLAYER_INPUT,
  steps: [
    {
      name: "static",
      frameIndex: 0,
      camera: STATIC_CAMERA,
      playerInput: STATIC_PLAYER_INPUT,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-smoke.png",
      },
    },
  ],
};

export const GENERATED_WORLD_TRANSITION_SCENARIO: GeneratedWorldSmokeScenario = {
  ...GENERATED_WORLD_SMOKE_SCENARIO,
  id: "transition",
  outputPath: "/tmp/mclone-deno-generated-world-transition-02-shifted.png",
  playerInput: TRANSITION_PLAYER_INPUT,
  steps: [
    {
      name: "initial",
      frameIndex: 0,
      camera: STATIC_CAMERA,
      playerInput: STATIC_PLAYER_INPUT,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-transition-01-initial.png",
      },
    },
    {
      name: "shifted",
      frameIndex: 1,
      camera: TRANSITION_CAMERA,
      playerInput: TRANSITION_PLAYER_INPUT,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-transition-02-shifted.png",
      },
    },
  ],
};

export const GENERATED_WORLD_TICK_CADENCE_SCENARIO: GeneratedWorldSmokeScenario = {
  ...GENERATED_WORLD_SMOKE_SCENARIO,
  id: "tick-cadence",
  outputPath: "/tmp/mclone-deno-generated-world-tick-cadence-04-release.png",
  playerInput: TICK_CADENCE_PLAYER_INPUTS[TICK_CADENCE_PLAYER_INPUTS.length - 1]!,
  cadence: {
    intervalMs: TICK_CADENCE_INTERVAL_MS,
    requirePlayerProgression: true,
  },
  steps: [
    {
      name: "tick-01-forward",
      frameIndex: 0,
      camera: STATIC_CAMERA,
      playerInput: TICK_CADENCE_PLAYER_INPUTS[0]!,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-tick-cadence-01-forward.png",
      },
    },
    {
      name: "tick-02-diagonal",
      frameIndex: 1,
      camera: STATIC_CAMERA,
      playerInput: TICK_CADENCE_PLAYER_INPUTS[1]!,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-tick-cadence-02-diagonal.png",
      },
    },
    {
      name: "tick-03-strafe",
      frameIndex: 2,
      camera: STATIC_CAMERA,
      playerInput: TICK_CADENCE_PLAYER_INPUTS[2]!,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-tick-cadence-03-strafe.png",
      },
    },
    {
      name: "tick-04-release",
      frameIndex: 3,
      camera: STATIC_CAMERA,
      playerInput: TICK_CADENCE_PLAYER_INPUTS[3]!,
      readback: {
        outputPath: "/tmp/mclone-deno-generated-world-tick-cadence-04-release.png",
      },
    },
  ],
};

export const GENERATED_WORLD_SMOKE_SCENARIOS = {
  static: GENERATED_WORLD_SMOKE_SCENARIO,
  transition: GENERATED_WORLD_TRANSITION_SCENARIO,
  tickCadence: GENERATED_WORLD_TICK_CADENCE_SCENARIO,
} as const;

export function getGeneratedWorldSmokeScenarioById(id: string | null | undefined): GeneratedWorldSmokeScenario {
  switch (id) {
    case GENERATED_WORLD_TRANSITION_SCENARIO.id:
      return GENERATED_WORLD_TRANSITION_SCENARIO;
    case GENERATED_WORLD_TICK_CADENCE_SCENARIO.id:
      return GENERATED_WORLD_TICK_CADENCE_SCENARIO;
    default:
      return GENERATED_WORLD_SMOKE_SCENARIO;
  }
}

export function getGeneratedWorldSmokeExpectedLoadedChunkCount(
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): number {
  return getExpectedLoadedChunkCount(scenario.viewDistance);
}

export function getGeneratedWorldSmokeCameraChunkCenter(camera: CameraState): GeneratedWorldSmokeChunkCenter {
  return {
    x: SectionPos.posToSectionCoord(camera.position.x),
    z: SectionPos.posToSectionCoord(camera.position.z),
  };
}

export function getGeneratedWorldSmokeScenarioSteps(
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): readonly GeneratedWorldSmokeScenarioStep[] {
  return scenario.steps;
}

export function getGeneratedWorldSmokeStepExpectedLoadedChunkCount(
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
): number {
  return step.expectedLoadedChunkCount ?? getGeneratedWorldSmokeExpectedLoadedChunkCount(scenario);
}

export function getGeneratedWorldSmokeStepExpectedChunkCenter(
  step: GeneratedWorldSmokeScenarioStep,
): GeneratedWorldSmokeChunkCenter {
  return step.expectedChunkCenter ?? getGeneratedWorldSmokeCameraChunkCenter(step.camera);
}

export function getGeneratedWorldSmokeStepOutputPath(
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
): string | undefined {
  return step.readback?.outputPath ?? (stepIndex === scenario.steps.length - 1 ? scenario.outputPath : undefined);
}

export function getGeneratedWorldSmokeExpectedPlayerInputSequence(
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): number | undefined {
  for (let index = scenario.steps.length - 1; index >= 0; index--) {
    const input = scenario.steps[index]?.playerInput;
    if (input !== undefined) {
      return input.sequence;
    }
  }

  return scenario.playerInput.sequence;
}

export function createGeneratedWorldSmokeSearchParams(
  extra: Record<string, string>,
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): URLSearchParams {
  const firstStep = scenario.steps[0] ?? {
    name: "static",
    camera: scenario.camera,
    playerInput: scenario.playerInput,
  };
  return new URLSearchParams({
    generatedWorldScenario: scenario.id,
    viewDistance: scenario.viewDistance.toString(),
    renderDistance: scenario.renderDistance.toString(),
    lightingMode: scenario.engineConfig.lightingMode,
    liquidSimulationMode: scenario.engineConfig.liquidSimulationMode,
    fogColor: scenario.fogColorHex,
    cameraX: firstStep.camera.position.x.toString(),
    cameraY: firstStep.camera.position.y.toString(),
    cameraZ: firstStep.camera.position.z.toString(),
    cameraYaw: firstStep.camera.yRot.toString(),
    cameraPitch: firstStep.camera.xRot.toString(),
    ...extra,
  });
}

export function validateGeneratedWorldSmokeResult(
  result: GeneratedWorldSmokeResultLike,
  options: GeneratedWorldSmokeValidationOptions = {},
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): string[] {
  const errors: string[] = [];
  const steps = getGeneratedWorldSmokeScenarioSteps(scenario);
  const finalStep = steps[steps.length - 1]!;
  validateGeneratedWorldSmokeResultAgainstStep(result, options, scenario, finalStep, errors);

  if (options.expectedWorldTransport !== undefined && result.worldTransport !== options.expectedWorldTransport) {
    errors.push(`expected worldTransport=${options.expectedWorldTransport}, got ${result.worldTransport ?? "undefined"}`);
  }

  const stepResults = result.steps;
  if (stepResults === undefined) {
    if (options.requireSteps) {
      errors.push("expected per-step results");
    }
  } else {
    if (stepResults.length !== steps.length) {
      errors.push(`expected ${steps.length.toString()} step results, got ${stepResults.length.toString()}`);
    }
    const stepCount = Math.min(stepResults.length, steps.length);
    for (let index = 0; index < stepCount; index++) {
      validateGeneratedWorldSmokeStepResult(stepResults[index]!, scenario, steps[index]!, index, options, errors);
    }
    if (scenario.cadence?.requirePlayerProgression === true) {
      validateGeneratedWorldSmokeStepProgression(stepResults, errors);
    }
  }

  return errors;
}

function validateGeneratedWorldSmokeStepResult(
  result: GeneratedWorldSmokeResultLike,
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
  options: GeneratedWorldSmokeValidationOptions,
  errors: string[],
): void {
  if (result.stepName !== step.name) {
    errors.push(`expected steps[${stepIndex.toString()}].stepName=${step.name}, got ${result.stepName ?? "undefined"}`);
  }
  if (result.stepIndex !== stepIndex) {
    errors.push(`expected steps[${stepIndex.toString()}].stepIndex=${stepIndex.toString()}, got ${result.stepIndex?.toString() ?? "undefined"}`);
  }
  if (step.frameIndex !== undefined && result.frameIndex !== step.frameIndex) {
    errors.push(`expected steps[${stepIndex.toString()}].frameIndex=${step.frameIndex.toString()}, got ${result.frameIndex?.toString() ?? "undefined"}`);
  }
  if (step.presentationDelayMs !== undefined && result.presentationDelayMs !== step.presentationDelayMs) {
    errors.push(`expected steps[${stepIndex.toString()}].presentationDelayMs=${step.presentationDelayMs.toString()}, got ${result.presentationDelayMs?.toString() ?? "undefined"}`);
  } else if (scenario.cadence !== undefined && result.presentationDelayMs !== scenario.cadence.intervalMs) {
    errors.push(`expected steps[${stepIndex.toString()}].presentationDelayMs=${scenario.cadence.intervalMs.toString()}, got ${result.presentationDelayMs?.toString() ?? "undefined"}`);
  }
  validateGeneratedWorldSmokeResultAgainstStep(result, options, scenario, step, errors, `steps[${stepIndex.toString()}].`);
}

function validateGeneratedWorldSmokeTopology(
  result: GeneratedWorldSmokeResultLike,
  options: GeneratedWorldSmokeValidationOptions,
  scenario: GeneratedWorldSmokeScenario,
  errors: string[],
  prefix = "",
): void {
  const worldHost = options.expectedWorldTransport === undefined
    ? undefined
    : worldTransportToRuntimeWorldHost(options.expectedWorldTransport);
  errors.push(...validateGeneratedWorldRuntimeTopology(
    result.topology,
    getGeneratedWorldRuntimeTopologyRequirement(scenario.engineConfig, worldHost),
    `${prefix}topology`,
  ));
}

function validateGeneratedWorldSmokeStepProgression(
  stepResults: readonly GeneratedWorldSmokeResultLike[],
  errors: string[],
): void {
  let previousInputSequence: number | undefined;
  let previousPlayerStateRevision: number | undefined;
  let previousPlayerTick: number | undefined;
  let firstPlayerPosition: readonly number[] | undefined;
  let lastPlayerPosition: readonly number[] | undefined;

  for (let index = 0; index < stepResults.length; index++) {
    const result = stepResults[index]!;
    const prefix = `steps[${index.toString()}].`;
    if (result.playerInputSequence === undefined) {
      errors.push(`${prefix}expected playerInputSequence for cadence progression`);
    } else if (previousInputSequence !== undefined && result.playerInputSequence <= previousInputSequence) {
      errors.push(`${prefix}expected playerInputSequence to advance past ${previousInputSequence.toString()}, got ${result.playerInputSequence.toString()}`);
    }
    if (result.playerStateRevision === undefined) {
      errors.push(`${prefix}expected playerStateRevision for cadence progression`);
    } else if (previousPlayerStateRevision !== undefined && result.playerStateRevision <= previousPlayerStateRevision) {
      errors.push(`${prefix}expected playerStateRevision to advance past ${previousPlayerStateRevision.toString()}, got ${result.playerStateRevision.toString()}`);
    }
    if (result.playerTick === undefined) {
      errors.push(`${prefix}expected playerTick for cadence progression`);
    } else if (previousPlayerTick !== undefined && result.playerTick <= previousPlayerTick) {
      errors.push(`${prefix}expected playerTick to advance past ${previousPlayerTick.toString()}, got ${result.playerTick.toString()}`);
    }

    previousInputSequence = result.playerInputSequence;
    previousPlayerStateRevision = result.playerStateRevision;
    previousPlayerTick = result.playerTick;

    if (result.playerPosition !== undefined) {
      firstPlayerPosition ??= result.playerPosition;
      lastPlayerPosition = result.playerPosition;
    }
  }

  if (
    firstPlayerPosition !== undefined
    && lastPlayerPosition !== undefined
    && firstPlayerPosition.length === 3
    && lastPlayerPosition.length === 3
    && firstPlayerPosition.every((value, index) => Math.abs(value - lastPlayerPosition![index]!) < 0.000_001)
  ) {
    errors.push("expected cadence playerPosition to change across steps");
  }
}

function validateGeneratedWorldSmokeResultAgainstStep(
  result: GeneratedWorldSmokeResultLike,
  options: GeneratedWorldSmokeValidationOptions,
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  errors: string[],
  prefix = "",
): void {
  const expectedLoadedChunkCount = getGeneratedWorldSmokeStepExpectedLoadedChunkCount(scenario, step);
  const expectedChunkCenter = getGeneratedWorldSmokeStepExpectedChunkCenter(step);
  const expectedInputSequence = step.playerInput?.sequence ?? getGeneratedWorldSmokeExpectedPlayerInputSequence(scenario);
  if (result.loadedChunkCount !== expectedLoadedChunkCount) {
    errors.push(`${prefix}expected loadedChunkCount=${expectedLoadedChunkCount.toString()}, got ${result.loadedChunkCount.toString()}`);
  }
  if (result.expectedLoadedChunkCount !== expectedLoadedChunkCount) {
    errors.push(`${prefix}expected expectedLoadedChunkCount=${expectedLoadedChunkCount.toString()}, got ${result.expectedLoadedChunkCount.toString()}`);
  }
  if (result.viewDistance !== scenario.viewDistance) {
    errors.push(`${prefix}expected viewDistance=${scenario.viewDistance.toString()}, got ${result.viewDistance.toString()}`);
  }
  if (result.renderDistance !== scenario.renderDistance) {
    errors.push(`${prefix}expected renderDistance=${scenario.renderDistance.toString()}, got ${result.renderDistance.toString()}`);
  }
  if (result.lightingMode !== scenario.engineConfig.lightingMode) {
    errors.push(`${prefix}expected lightingMode=${scenario.engineConfig.lightingMode}, got ${result.lightingMode}`);
  }
  if (result.liquidSimulationMode !== scenario.engineConfig.liquidSimulationMode) {
    errors.push(`${prefix}expected liquidSimulationMode=${scenario.engineConfig.liquidSimulationMode}, got ${result.liquidSimulationMode}`);
  }
  validateGeneratedWorldSmokeTopology(result, options, scenario, errors, prefix);
  validateChunkCenter(`${prefix}expectedChunkCenter`, result.expectedChunkCenter, expectedChunkCenter, errors);
  validateChunkCenter(`${prefix}chunkCenter`, result.chunkCenter, expectedChunkCenter, errors);
  if (result.sessionChunkCenter !== undefined) {
    validateChunkCenter(`${prefix}sessionChunkCenter`, result.sessionChunkCenter, expectedChunkCenter, errors);
  }
  if (result.sessionChunkRadius !== undefined && result.sessionChunkRadius !== scenario.viewDistance) {
    errors.push(`${prefix}expected sessionChunkRadius=${scenario.viewDistance.toString()}, got ${result.sessionChunkRadius.toString()}`);
  }
  if (result.solidDrawCount <= 0) {
    errors.push(`${prefix}expected at least one solid draw`);
  }
  if (result.renderWorldCounters.ingestBatchCount <= 0) {
    errors.push(`${prefix}expected at least one render-world ingest batch`);
  }
  if (result.renderWorldCounters.meshBuildRequestCount <= 0) {
    errors.push(`${prefix}expected at least one render-world mesh build request`);
  }
  if (result.renderWorldCounters.meshCompletionCount <= 0) {
    errors.push(`${prefix}expected at least one render-world mesh completion`);
  }
  if (result.renderWorldCounters.mainThreadGpuUploadCount <= 0) {
    errors.push(`${prefix}expected at least one main-thread GPU upload`);
  }
  if (result.renderWorldCounters.meshNotReadyResponseCount < 0) {
    errors.push(`${prefix}expected non-negative mesh-not-ready count`);
  }
  if (result.renderQueueStats.renderedChunkCount <= 0) {
    errors.push(`${prefix}expected rendered chunks`);
  }
  if (result.renderQueueStats.pendingVisibleChunkCompileCount !== 0) {
    errors.push(`${prefix}expected no pending visible chunk compiles, got ${result.renderQueueStats.pendingVisibleChunkCompileCount.toString()}`);
  }
  if (result.renderQueueStats.queuedChunkBuildCount !== 0) {
    errors.push(`${prefix}expected no queued chunk builds, got ${result.renderQueueStats.queuedChunkBuildCount.toString()}`);
  }
  if (result.renderQueueStats.activeChunkBuildCount !== 0) {
    errors.push(`${prefix}expected no active chunk builds, got ${result.renderQueueStats.activeChunkBuildCount.toString()}`);
  }

  if (options.requireReadback || step.readback !== undefined) {
    const minimumNonClearPixels = step.readback?.minimumNonClearPixels ?? scenario.minimumNonClearPixels;
    if (result.nonClearPixels === undefined) {
      errors.push(`${prefix}expected nonClearPixels in readback result`);
    } else if (result.nonClearPixels < minimumNonClearPixels) {
      errors.push(`${prefix}expected at least ${minimumNonClearPixels.toString()} non-clear pixels, got ${result.nonClearPixels.toString()}`);
    }
    validatePixel(`${prefix}centerPixel`, result.centerPixel, errors);
    validatePixel(`${prefix}terrainPixel`, result.terrainPixel, errors);
  }

  if (options.requirePlayerInput && expectedInputSequence !== undefined) {
    if (result.sessionId === undefined) {
      errors.push(`${prefix}expected sessionId`);
    }
    if (result.playerId === undefined) {
      errors.push(`${prefix}expected playerId`);
    }
    if (result.sessionId !== undefined && result.playerId !== undefined && result.sessionId === result.playerId) {
      errors.push(`${prefix}expected playerId to differ from sessionId`);
    }
    if (result.playerName !== "Player") {
      errors.push(`${prefix}expected playerName=Player, got ${result.playerName ?? "undefined"}`);
    }
    if ((result.sessionRevision ?? 0) <= 0) {
      errors.push(`${prefix}expected positive sessionRevision`);
    }
    if (result.expectedPlayerInputSequence !== undefined && result.expectedPlayerInputSequence !== expectedInputSequence) {
      errors.push(`${prefix}expected expectedPlayerInputSequence=${expectedInputSequence.toString()}, got ${result.expectedPlayerInputSequence.toString()}`);
    }
    if (result.playerInputSequence !== expectedInputSequence) {
      errors.push(`${prefix}expected playerInputSequence=${expectedInputSequence.toString()}, got ${result.playerInputSequence?.toString() ?? "undefined"}`);
    }
    if ((result.playerStateRevision ?? 0) <= 0) {
      errors.push(`${prefix}expected positive playerStateRevision`);
    }
    if ((result.playerTick ?? 0) <= 0) {
      errors.push(`${prefix}expected positive playerTick`);
    }
    if (result.playerPosition?.length !== 3) {
      errors.push(`${prefix}expected 3D playerPosition`);
    }
  }
}

function validateChunkCenter(
  label: string,
  actual: GeneratedWorldSmokeChunkCenter | undefined,
  expected: GeneratedWorldSmokeChunkCenter,
  errors: string[],
): void {
  if (actual === undefined) {
    errors.push(`expected ${label}`);
    return;
  }
  if (actual.x !== expected.x || actual.z !== expected.z) {
    errors.push(`expected ${label}=(${expected.x.toString()},${expected.z.toString()}), got (${actual.x.toString()},${actual.z.toString()})`);
  }
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
