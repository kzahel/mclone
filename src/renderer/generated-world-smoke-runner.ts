import type { ClientRuntime } from "../runtime/client/client-runtime";
import type { CameraState } from "./game-renderer";
import type { LevelRenderFrame } from "./level-renderer";
import { RenderType } from "./render-type";
import {
  applyRenderWorldDirtySections,
  getSceneLoadedChunkCount,
  getSceneRenderQueueStats,
  getSceneRenderWorldPerformanceCounters,
  renderSceneUntilSettled,
  SCENE_DEPTH_FORMAT,
  waitForLoadedChunkRing,
  type RenderSceneQueueStats,
  type RenderWorldPerformanceCounters,
  type RendererScene,
} from "./scene-setup";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  getGeneratedWorldSmokeCameraChunkCenter,
  getGeneratedWorldSmokeExpectedPlayerInputSequence,
  getGeneratedWorldSmokeScenarioSteps,
  getGeneratedWorldSmokeStepExpectedChunkCenter,
  getGeneratedWorldSmokeStepExpectedLoadedChunkCount,
  getGeneratedWorldSmokeStepOutputPath,
  type GeneratedWorldSmokeChunkCenter,
  type GeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeScenarioStep,
  type GeneratedWorldSmokeResultLike,
} from "./generated-world-smoke-scenario";
import { countPixelsDifferentFrom, readPixel, rgbaColorToPixel } from "./static-frame-harness";

export type GeneratedWorldSmokeWorldTransport = "worker" | "remote";
export type RgbaPixel = readonly [number, number, number, number];

export interface GeneratedWorldSmokeFrameResult {
  readonly frame: LevelRenderFrame;
  readonly chunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedChunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedLoadedChunkCount: number;
  readonly loadedChunkCount: number;
  readonly renderWorldCounters: RenderWorldPerformanceCounters;
  readonly renderQueueStats: RenderSceneQueueStats;
  readonly solidDrawCount: number;
  readonly cutoutDrawCount: number;
  readonly translucentDrawCount: number;
}

export interface GeneratedWorldSmokeTargetReadback {
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  readonly pixels?: Uint8Array;
  readonly centerPixel?: readonly number[];
  readonly terrainPixel?: readonly number[];
  readonly clearPixel?: readonly number[];
  readonly nonClearPixels?: number;
  readonly byteLength?: number;
}

export interface GeneratedWorldSmokeReadback {
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  readonly pixels?: Uint8Array;
  readonly centerPixel: RgbaPixel;
  readonly terrainPixel: RgbaPixel;
  readonly clearPixel: RgbaPixel;
  readonly nonClearPixels: number;
  readonly byteLength: number;
}

export interface GeneratedWorldSmokeRenderTargetContext {
  readonly scene: RendererScene;
  readonly frame: LevelRenderFrame;
  readonly scenario: GeneratedWorldSmokeScenario;
  readonly step: GeneratedWorldSmokeScenarioStep;
  readonly stepIndex: number;
  readonly clearPixel: RgbaPixel;
}

export interface GeneratedWorldSmokeRenderTarget {
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  renderFrame(context: GeneratedWorldSmokeRenderTargetContext): Promise<GeneratedWorldSmokeTargetReadback>;
}

export interface GeneratedWorldSmokeScenarioResult extends GeneratedWorldSmokeResultLike {
  readonly ok: true;
  readonly worldTransport: GeneratedWorldSmokeWorldTransport;
  readonly meshTransport: "worker";
  readonly saveId: string;
  readonly stepName: string;
  readonly stepIndex: number;
  readonly chunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedChunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkRadius?: number;
  readonly expectedPlayerInputSequence?: number;
  readonly format: GPUTextureFormat;
  readonly adapterInfo: string;
  readonly outputPath?: string;
  readonly width: number;
  readonly height: number;
  readonly byteLength: number;
  readonly centerPixel: RgbaPixel;
  readonly terrainPixel: RgbaPixel;
  readonly clearPixel: RgbaPixel;
  readonly nonClearPixels: number;
  readonly cutoutDrawCount: number;
  readonly translucentDrawCount: number;
  readonly playerPosition?: readonly [number, number, number];
  readonly steps: readonly GeneratedWorldSmokeStepResult[];
}

export interface GeneratedWorldSmokeStepResult extends GeneratedWorldSmokeResultLike {
  readonly worldTransport: GeneratedWorldSmokeWorldTransport;
  readonly meshTransport: "worker";
  readonly stepName: string;
  readonly stepIndex: number;
  readonly chunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedChunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkCenter?: GeneratedWorldSmokeChunkCenter;
  readonly sessionChunkRadius?: number;
  readonly expectedPlayerInputSequence?: number;
  readonly format: GPUTextureFormat;
  readonly outputPath?: string;
  readonly width: number;
  readonly height: number;
  readonly byteLength: number;
  readonly centerPixel: RgbaPixel;
  readonly terrainPixel: RgbaPixel;
  readonly clearPixel: RgbaPixel;
  readonly nonClearPixels: number;
  readonly cutoutDrawCount: number;
  readonly translucentDrawCount: number;
  readonly playerPosition?: readonly [number, number, number];
}

export interface GeneratedWorldSmokeRunOptions {
  readonly scene: RendererScene;
  readonly target: GeneratedWorldSmokeRenderTarget;
  readonly worldTransport: GeneratedWorldSmokeWorldTransport;
  readonly scenario?: GeneratedWorldSmokeScenario;
  readonly camera?: CameraState;
  readonly meshTransport?: "worker";
  readonly format?: GPUTextureFormat;
  readonly adapterInfo?: string;
  readonly outputPath?: string;
  readonly lightingMode?: string;
  readonly liquidSimulationMode?: string;
  readonly validatePipelines?: boolean;
  readonly validatePipelineFormats?: readonly GPUTextureFormat[];
}

export interface GeneratedWorldSmokeRun {
  readonly frame: LevelRenderFrame;
  readonly readback: GeneratedWorldSmokeReadback;
  readonly stepRuns: readonly GeneratedWorldSmokeStepRun[];
  readonly result: GeneratedWorldSmokeScenarioResult;
}

export interface GeneratedWorldSmokeStepRun {
  readonly step: GeneratedWorldSmokeScenarioStep;
  readonly frame: LevelRenderFrame;
  readonly readback: GeneratedWorldSmokeReadback;
  readonly result: GeneratedWorldSmokeStepResult;
}

interface PreparedGeneratedWorldSmokeStep {
  readonly chunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedChunkCenter: GeneratedWorldSmokeChunkCenter;
  readonly expectedLoadedChunkCount: number;
}

const VALIDATED_RENDER_TYPES = [
  RenderType.solid(),
  RenderType.cutoutMipped(),
  RenderType.cutout(),
  RenderType.translucent(),
  RenderType.translucentMovingBlock(),
  RenderType.translucentNoCrumbling(),
  RenderType.tripwire(),
  RenderType.lines(),
  RenderType.lineStrip(),
] as const;

export async function renderGeneratedWorldSmokeFrame(
  scene: RendererScene,
  camera: CameraState,
  expectedLoadedChunkCount: number,
): Promise<GeneratedWorldSmokeFrameResult> {
  const prepared = await prepareGeneratedWorldSmokeStep(scene, camera, expectedLoadedChunkCount, getGeneratedWorldSmokeCameraChunkCenter(camera));

  // Mesh requests issued before the full chunk ring arrives can legitimately
  // return "not ready"; force one visibility pass after the ring is present.
  scene.levelRenderer.allChanged();
  return collectGeneratedWorldSmokeFrameResult(
    scene,
    await renderSceneUntilSettled(scene, camera),
    prepared,
  );
}

async function prepareGeneratedWorldSmokeStep(
  scene: RendererScene,
  camera: CameraState,
  expectedLoadedChunkCount: number,
  expectedChunkCenter: GeneratedWorldSmokeChunkCenter,
): Promise<PreparedGeneratedWorldSmokeStep> {
  const chunkCenter = getGeneratedWorldSmokeCameraChunkCenter(camera);
  if (await scene.clientRuntime.setChunkInterest({
    type: "set_chunk_view",
    centerChunkX: chunkCenter.x,
    centerChunkZ: chunkCenter.z,
    radius: scene.viewDistance,
  })) {
    applyRenderWorldDirtySections(scene);
    scene.levelRenderer.allChanged();
  }

  if (!await waitForLoadedChunkRing(scene, expectedLoadedChunkCount, { maxAttempts: 1800 })) {
    throw new Error(
      `expected ${expectedLoadedChunkCount.toString()} loaded chunks for viewDistance=${scene.viewDistance.toString()}, got ${getSceneLoadedChunkCount(scene).toString()}`,
    );
  }
  assertSessionChunkView(scene, chunkCenter);

  return {
    chunkCenter,
    expectedChunkCenter,
    expectedLoadedChunkCount,
  };
}

export async function runGeneratedWorldSmokeScenario(
  options: GeneratedWorldSmokeRunOptions,
): Promise<GeneratedWorldSmokeRun> {
  const scenario = options.scenario ?? GENERATED_WORLD_SMOKE_SCENARIO;
  const steps = resolveGeneratedWorldSmokeRunSteps(scenario, options.camera);
  if (steps.length <= 0) {
    throw new Error(`generated-world smoke scenario ${scenario.id} has no steps`);
  }

  if (options.validatePipelines !== false) {
    await validateGeneratedWorldSmokePipelines(
      options.scene,
      options.validatePipelineFormats ?? [options.format ?? options.scene.format, options.target.format],
    );
  }

  const stepRuns: GeneratedWorldSmokeStepRun[] = [];
  let expectedPlayerInputSequence: number | undefined;
  for (let stepIndex = 0; stepIndex < steps.length; stepIndex++) {
    const step = steps[stepIndex]!;
    if (step.playerInput !== undefined) {
      expectedPlayerInputSequence = step.playerInput.sequence;
    }
    stepRuns.push(await runGeneratedWorldSmokeScenarioStep(
      options,
      scenario,
      step,
      stepIndex,
      expectedPlayerInputSequence,
    ));
  }

  const lastRun = stepRuns[stepRuns.length - 1]!;

  return {
    frame: lastRun.frame,
    readback: lastRun.readback,
    stepRuns,
    result: {
      ...lastRun.result,
      ok: true,
      saveId: options.scene.saveMetadata.saveId,
      adapterInfo: options.adapterInfo ?? describeGpuAdapter(options.scene.adapter),
      steps: stepRuns.map((run) => run.result),
    },
  };
}

async function runGeneratedWorldSmokeScenarioStep(
  options: GeneratedWorldSmokeRunOptions,
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
  expectedPlayerInputSequence: number | undefined,
): Promise<GeneratedWorldSmokeStepRun> {
  const expectedLoadedChunkCount = getGeneratedWorldSmokeStepExpectedLoadedChunkCount(scenario, step);
  const prepared = await prepareGeneratedWorldSmokeStep(
    options.scene,
    step.camera,
    expectedLoadedChunkCount,
    getGeneratedWorldSmokeStepExpectedChunkCenter(step),
  );
  if (step.playerInput !== undefined) {
    await runGeneratedWorldSmokePlayerInputCommand(options.scene, step.playerInput);
  }

  options.scene.levelRenderer.allChanged();
  const frameResult = collectGeneratedWorldSmokeFrameResult(
    options.scene,
    await renderSceneUntilSettled(options.scene, step.camera),
    prepared,
  );
  if (frameResult.solidDrawCount <= 0) {
    throw new Error(`generated-world smoke step ${step.name} produced no solid drawables for the generated terrain scene`);
  }

  const readback = await renderGeneratedWorldSmokeTarget(
    options.scene,
    frameResult.frame,
    scenario,
    step,
    stepIndex,
    options.target,
  );
  return {
    step,
    frame: frameResult.frame,
    readback,
    result: createGeneratedWorldSmokeStepResult(
      options,
      scenario,
      step,
      stepIndex,
      frameResult,
      readback,
      expectedPlayerInputSequence,
    ),
  };
}

export async function runGeneratedWorldSmokePlayerInput(
  scene: RendererScene,
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): Promise<void> {
  await runGeneratedWorldSmokePlayerInputCommand(scene, scenario.playerInput);
}

export async function runGeneratedWorldSmokePlayerInputForRuntime(
  clientRuntime: ClientRuntime,
  onTransportDrained?: () => void,
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): Promise<void> {
  await runGeneratedWorldSmokePlayerInputCommandForRuntime(clientRuntime, scenario.playerInput, onTransportDrained);
}

async function runGeneratedWorldSmokePlayerInputCommand(
  scene: RendererScene,
  input: GeneratedWorldSmokeScenarioStep["playerInput"],
): Promise<void> {
  if (input === undefined) {
    return;
  }

  await runGeneratedWorldSmokePlayerInputCommandForRuntime(scene.clientRuntime, input, () => {
    applyRenderWorldDirtySections(scene);
  });
}

async function runGeneratedWorldSmokePlayerInputCommandForRuntime(
  clientRuntime: ClientRuntime,
  input: NonNullable<GeneratedWorldSmokeScenarioStep["playerInput"]>,
  onTransportDrained?: () => void,
): Promise<void> {
  const initialPlayerState = clientRuntime.publishPresentationState().localPlayerState;
  if (initialPlayerState === undefined) {
    return;
  }

  await clientRuntime.sendPlayerCommand({
    type: "set_player_input",
    input,
  });
  for (let attempt = 0; attempt < 5; attempt++) {
    await sleep(60);
    if (await clientRuntime.drainTransportUpdates()) {
      onTransportDrained?.();
    }
    const updatedPlayerState = clientRuntime.publishPresentationState().localPlayerState;
    if (
      updatedPlayerState !== undefined
      && updatedPlayerState.revision > initialPlayerState.revision
      && updatedPlayerState.acknowledgedInputSequence >= input.sequence
    ) {
      break;
    }
  }
}

export async function validateGeneratedWorldSmokePipelines(
  scene: RendererScene,
  colorFormats: readonly GPUTextureFormat[],
): Promise<void> {
  scene.device.pushErrorScope("validation");
  for (const colorFormat of new Set(colorFormats)) {
    for (const renderType of VALIDATED_RENDER_TYPES) {
      scene.pipelineCache.getOrCreate(renderType, colorFormat, SCENE_DEPTH_FORMAT);
    }
  }

  const pipelineError = await popValidationError(scene.device, "WebGPU pipeline validation failed");
  if (pipelineError !== undefined) {
    throw new Error(pipelineError);
  }
}

export function describeGpuAdapter(adapter: GPUAdapter | undefined): string {
  const info = adapter?.info;
  return [info?.vendor, info?.architecture, info?.device, info?.description]
    .filter(Boolean)
    .join(" / ") || "unknown";
}

function resolveGeneratedWorldSmokeRunSteps(
  scenario: GeneratedWorldSmokeScenario,
  cameraOverride: CameraState | undefined,
): readonly GeneratedWorldSmokeScenarioStep[] {
  const steps = getGeneratedWorldSmokeScenarioSteps(scenario);
  if (cameraOverride === undefined || steps.length !== 1) {
    return steps;
  }

  const step = steps[0]!;
  return [{
    ...step,
    camera: cameraOverride,
    expectedChunkCenter: getGeneratedWorldSmokeCameraChunkCenter(cameraOverride),
  }];
}

function assertSessionChunkView(scene: RendererScene, chunkCenter: GeneratedWorldSmokeChunkCenter): void {
  const chunkView = scene.clientRuntime.publishPresentationState().sessionState?.chunkView;
  if (chunkView === undefined) {
    throw new Error("generated-world smoke expected session chunk view after set_chunk_view");
  }
  if (
    chunkView.centerChunkX !== chunkCenter.x
    || chunkView.centerChunkZ !== chunkCenter.z
    || chunkView.radius !== scene.viewDistance
  ) {
    throw new Error(
      `generated-world smoke expected session chunk view (${chunkCenter.x.toString()},${chunkCenter.z.toString()},r=${scene.viewDistance.toString()}), got (${chunkView.centerChunkX.toString()},${chunkView.centerChunkZ.toString()},r=${chunkView.radius.toString()})`,
    );
  }
}

function createGeneratedWorldSmokeStepResult(
  options: GeneratedWorldSmokeRunOptions,
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
  frameResult: GeneratedWorldSmokeFrameResult,
  readback: GeneratedWorldSmokeReadback,
  expectedPlayerInputSequence: number | undefined,
): GeneratedWorldSmokeStepResult {
  const presentation = options.scene.clientRuntime.publishPresentationState();
  const sessionState = presentation.sessionState;
  const sessionChunkView = sessionState?.chunkView;
  const playerState = presentation.localPlayerState;
  const scenarioOutputPath = getGeneratedWorldSmokeStepOutputPath(scenario, step, stepIndex);
  const isFinalScenarioStep = stepIndex === getGeneratedWorldSmokeScenarioSteps(scenario).length - 1;

  return {
    worldTransport: options.worldTransport,
    meshTransport: options.meshTransport ?? "worker",
    stepName: step.name,
    stepIndex,
    chunkCenter: frameResult.chunkCenter,
    expectedChunkCenter: frameResult.expectedChunkCenter,
    sessionChunkCenter: sessionChunkView === undefined
      ? undefined
      : { x: sessionChunkView.centerChunkX, z: sessionChunkView.centerChunkZ },
    sessionChunkRadius: sessionChunkView?.radius,
    expectedPlayerInputSequence: expectedPlayerInputSequence ?? getGeneratedWorldSmokeExpectedPlayerInputSequence(scenario),
    outputPath: isFinalScenarioStep ? options.outputPath ?? scenarioOutputPath : scenarioOutputPath,
    width: readback.width,
    height: readback.height,
    format: options.format ?? readback.format,
    expectedLoadedChunkCount: frameResult.expectedLoadedChunkCount,
    loadedChunkCount: frameResult.loadedChunkCount,
    viewDistance: options.scene.viewDistance,
    renderDistance: options.scene.gameRenderer.getRenderDistance(),
    lightingMode: options.lightingMode ?? scenario.engineConfig.lightingMode,
    liquidSimulationMode: options.liquidSimulationMode ?? scenario.engineConfig.liquidSimulationMode,
    solidDrawCount: frameResult.solidDrawCount,
    cutoutDrawCount: frameResult.cutoutDrawCount,
    translucentDrawCount: frameResult.translucentDrawCount,
    nonClearPixels: readback.nonClearPixels,
    centerPixel: readback.centerPixel,
    terrainPixel: readback.terrainPixel,
    clearPixel: readback.clearPixel,
    byteLength: readback.byteLength,
    renderWorldCounters: frameResult.renderWorldCounters,
    renderQueueStats: frameResult.renderQueueStats,
    sessionId: sessionState?.sessionId,
    playerId: sessionState?.playerId,
    playerName: sessionState?.playerProfile.name,
    playerProfileId: sessionState?.playerProfile.profileId,
    sessionRevision: sessionState?.revision,
    playerInputSequence: playerState?.acknowledgedInputSequence,
    playerStateRevision: playerState?.revision,
    playerTick: playerState?.tick,
    playerPosition: playerState ? [playerState.position.x, playerState.position.y, playerState.position.z] as const : undefined,
  };
}

function collectGeneratedWorldSmokeFrameResult(
  scene: RendererScene,
  frame: LevelRenderFrame,
  prepared: PreparedGeneratedWorldSmokeStep,
): GeneratedWorldSmokeFrameResult {
  return {
    frame,
    chunkCenter: prepared.chunkCenter,
    expectedChunkCenter: prepared.expectedChunkCenter,
    expectedLoadedChunkCount: prepared.expectedLoadedChunkCount,
    loadedChunkCount: getSceneLoadedChunkCount(scene),
    renderWorldCounters: getSceneRenderWorldPerformanceCounters(scene),
    renderQueueStats: getSceneRenderQueueStats(scene),
    solidDrawCount: frame.layerDraws.get(RenderType.solid())?.length ?? 0,
    cutoutDrawCount: frame.layerDraws.get(RenderType.cutout())?.length ?? 0,
    translucentDrawCount: frame.layerDraws.get(RenderType.translucent())?.length ?? 0,
  };
}

async function renderGeneratedWorldSmokeTarget(
  scene: RendererScene,
  frame: LevelRenderFrame,
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
  target: GeneratedWorldSmokeRenderTarget,
): Promise<GeneratedWorldSmokeReadback> {
  const clearPixel = tupleFromPixel(rgbaColorToPixel(frame.fogColor));
  scene.device.pushErrorScope("validation");
  let readback: GeneratedWorldSmokeTargetReadback;
  try {
    readback = await target.renderFrame({ scene, frame, scenario, step, stepIndex, clearPixel });
  } catch (error) {
    const validationError = await popValidationError(scene.device, "WebGPU submission validation failed");
    if (validationError !== undefined) {
      throw new Error(validationError);
    }
    throw error;
  }

  const submissionError = await popValidationError(scene.device, "WebGPU submission validation failed");
  if (submissionError !== undefined) {
    throw new Error(submissionError);
  }

  return normalizeGeneratedWorldSmokeReadback(readback, clearPixel);
}

function normalizeGeneratedWorldSmokeReadback(
  readback: GeneratedWorldSmokeTargetReadback,
  fallbackClearPixel: RgbaPixel,
): GeneratedWorldSmokeReadback {
  const clearPixel = tupleFromPixel(readback.clearPixel ?? fallbackClearPixel);
  const clearPixelBytes = new Uint8Array(clearPixel);
  const centerPixel = tupleFromPixel(readback.centerPixel ?? readOptionalPixel(readback, Math.floor(readback.width / 2), Math.floor(readback.height / 2)));
  const terrainPixel = tupleFromPixel(readback.terrainPixel ?? readOptionalPixel(readback, Math.floor(readback.width / 2), Math.floor((readback.height * 3) / 4)));
  return {
    width: readback.width,
    height: readback.height,
    format: readback.format,
    pixels: readback.pixels,
    centerPixel,
    terrainPixel,
    clearPixel,
    nonClearPixels: readback.nonClearPixels ?? (readback.pixels === undefined ? 0 : countPixelsDifferentFrom(readback.pixels, clearPixelBytes, 6)),
    byteLength: readback.byteLength ?? readback.pixels?.byteLength ?? 0,
  };
}

function readOptionalPixel(readback: GeneratedWorldSmokeTargetReadback, x: number, y: number): ArrayLike<number> | undefined {
  return readback.pixels === undefined ? undefined : readPixel(readback.pixels, readback.width, x, y);
}

function tupleFromPixel(pixel: ArrayLike<number> | undefined): RgbaPixel {
  return [pixel?.[0] ?? 0, pixel?.[1] ?? 0, pixel?.[2] ?? 0, pixel?.[3] ?? 0];
}

async function popValidationError(device: GPUDevice, label: string): Promise<string | undefined> {
  const error = await device.popErrorScope();
  return error ? `${label}: ${error.message}` : undefined;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
